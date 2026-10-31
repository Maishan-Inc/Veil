use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use veil_core::{AppError, AppResult, ErrorCode};
use veil_runtime::{
    fetch_releases, install_pipeline, select_candidates, EngineManifest, FlagDialect, InstallError,
    InstalledRuntime, PipelineError, Progress, ProviderError, RuntimeCandidate,
    RuntimeRegistryFile,
};

/// Serializable mirror of [`InstalledRuntime`] in camelCase.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVersionPayload {
    pub version: String,
    pub channel: String,
    pub executable_path: String,
    pub installed_at: String,
    pub sha256: String,
    pub size_bytes: Option<u64>,
}

impl From<&InstalledRuntime> for RuntimeVersionPayload {
    fn from(rt: &InstalledRuntime) -> Self {
        Self {
            version: rt.version.clone(),
            channel: rt.channel.clone(),
            executable_path: rt.executable_path.to_string_lossy().into_owned(),
            installed_at: rt.installed_at.clone(),
            sha256: rt.sha256.clone(),
            size_bytes: rt.size_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuntimeState {
    Missing,
    #[serde(rename_all = "camelCase")]
    Installed {
        current: RuntimeVersionPayload,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableRuntimePayload {
    pub version: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: Option<u64>,
    pub published_at: Option<String>,
}

impl From<&RuntimeCandidate> for AvailableRuntimePayload {
    fn from(c: &RuntimeCandidate) -> Self {
        Self {
            version: c.version.clone(),
            asset_name: c.asset_name.clone(),
            download_url: c.download_url.clone(),
            sha256: c.sha256.clone(),
            size_bytes: c.size_bytes,
            published_at: c.published_at.clone(),
        }
    }
}

/// Live install progress event, mirrors [`veil_runtime::Progress`] in serializable form.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuntimeProgress {
    #[serde(rename_all = "camelCase")]
    Downloading {
        downloaded: u64,
        total: Option<u64>,
        bytes_per_sec: u64,
    },
    Verifying,
    #[serde(rename_all = "camelCase")]
    Extracting {
        entries_done: u32,
        entries_total: u32,
    },
    Finalizing,
    #[serde(rename_all = "camelCase")]
    Completed {
        version: String,
    },
    #[serde(rename_all = "camelCase")]
    Cancelled {
        version: String,
    },
    #[serde(rename_all = "camelCase")]
    Failed {
        version: String,
        code: ErrorCode,
    },
}

impl From<&Progress> for RuntimeProgress {
    fn from(value: &Progress) -> Self {
        match value {
            Progress::Downloading {
                downloaded,
                total,
                bytes_per_sec,
            } => RuntimeProgress::Downloading {
                downloaded: *downloaded,
                total: *total,
                bytes_per_sec: *bytes_per_sec,
            },
            Progress::Verifying => RuntimeProgress::Verifying,
            Progress::Extracting {
                entries_done,
                entries_total,
            } => RuntimeProgress::Extracting {
                entries_done: *entries_done,
                entries_total: *entries_total,
            },
            Progress::Finalizing => RuntimeProgress::Finalizing,
            // `Completed` carries the version at the service layer (the pipeline event has none).
            Progress::Completed => RuntimeProgress::Completed {
                version: String::new(),
            },
        }
    }
}

/// What the fingerprint surface must know about the engine that would actually launch.
///
/// Every field describes the **currently selected** installed engine, not the manifest. The manifest
/// says what Veil would install today; the registry says what is on disk. Between the dialect flip
/// (2026-08-23) and this type, the two were conflated, so an engine installed under an older manifest
/// was reported with the new manifest's capabilities — five patched switches and no degraded banner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineCapability {
    /// Name of the selected engine as recorded at its install, falling back to the manifest's name
    /// when nothing is selected or nothing was recorded. It is the name the banner puts in front of
    /// the user, so it has to describe the same engine as `flag_dialect`.
    pub engine_name: String,
    /// The dialect `to_engine_args` is gated on for a launch right now. `stock-chromium` whenever the
    /// selection cannot be proven patched, including when there is no selection at all.
    pub flag_dialect: FlagDialect,
    /// Whether `flag_dialect` was read off the installed engine. `false` means *unknown*, which
    /// behaves like `stock-chromium` and must not be *described* as `stock-chromium`.
    pub dialect_recorded: bool,
}

/// Everything `list_installed_runtimes` reports, resolved from one registry read so the capability
/// cannot describe a different registry state than the list beside it.
pub struct InstalledRuntimesSnapshot {
    pub installed: Vec<RuntimeVersionPayload>,
    pub current_version: Option<String>,
    pub capability: EngineCapability,
}

/// The two paths that move together when the user changes the engine install location.
#[derive(Debug, Clone)]
struct RuntimeRoots {
    registry_path: PathBuf,
    install_root: PathBuf,
}

impl RuntimeRoots {
    fn from_registry_path(registry_path: PathBuf) -> Self {
        let install_root = registry_path
            .parent()
            .map(|p| p.join("versions"))
            .unwrap_or_else(|| PathBuf::from("versions"));
        Self {
            registry_path,
            install_root,
        }
    }
}

/// Engine runtime manager backing the IPC layer.
#[derive(Clone)]
pub struct RuntimeService {
    manifest: EngineManifest,
    /// Behind a lock because the install location is user-changeable at runtime, and every clone
    /// of this service — including the ones handed to `spawn_blocking` — must see the new root
    /// rather than the one captured when it was cloned.
    roots: Arc<RwLock<RuntimeRoots>>,
    cancels: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl RuntimeService {
    pub fn new(registry_path: PathBuf) -> Self {
        Self {
            manifest: EngineManifest::default(),
            roots: Arc::new(RwLock::new(RuntimeRoots::from_registry_path(registry_path))),
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn registry_path(&self) -> PathBuf {
        self.roots
            .read()
            .expect("runtime roots lock poisoned")
            .registry_path
            .clone()
    }

    pub(crate) fn install_root(&self) -> PathBuf {
        self.roots
            .read()
            .expect("runtime roots lock poisoned")
            .install_root
            .clone()
    }

    /// Point the service at a new `registry.json`. The caller is responsible for having moved the
    /// files first — this only changes where the service looks.
    pub fn set_registry_path(&self, registry_path: PathBuf) {
        *self.roots.write().expect("runtime roots lock poisoned") =
            RuntimeRoots::from_registry_path(registry_path);
    }

    /// Rewrite the stored executable paths after the install root moved.
    ///
    /// `InstalledRuntime::executable_path` is absolute, so a moved directory leaves every entry
    /// pointing at a file that no longer exists — the registry would list versions that cannot
    /// launch. Called after [`Self::set_registry_path`], so `self.install_root()` is the new root.
    ///
    /// Entries that do not sit under `old_install_root` are rebuilt from the version directory
    /// instead of being left alone: a path that matches neither root is already broken, and
    /// `<install root>/<version>/<file name>` is the layout the install pipeline produces.
    pub fn rebase_installed_paths(&self, old_install_root: &Path) -> AppResult<()> {
        let mut registry = self.load_registry()?;
        let new_root = self.install_root();
        for entry in &mut registry.installed {
            let relative = entry
                .executable_path
                .strip_prefix(old_install_root)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| {
                    let file_name = entry
                        .executable_path
                        .file_name()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("chrome.exe"));
                    PathBuf::from(&entry.version).join(file_name)
                });
            entry.executable_path = new_root.join(relative);
        }
        self.save_registry(&registry)
    }

    /// Override the manifest (tests).
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_manifest(mut self, manifest: EngineManifest) -> Self {
        self.manifest = manifest;
        self
    }

    /// Absolute directory where browser engine versions are installed.
    pub fn install_root_path(&self) -> String {
        self.install_root().to_string_lossy().into_owned()
    }

    fn load_registry(&self) -> AppResult<RuntimeRegistryFile> {
        RuntimeRegistryFile::load(&self.registry_path())
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseReadFailed))
    }

    fn save_registry(&self, registry: &RuntimeRegistryFile) -> AppResult<()> {
        registry
            .save(&self.registry_path())
            .map_err(|_| AppError::from(ErrorCode::RuntimeRegistryWriteFailed))
    }

    /// Current state of the active runtime.
    pub fn state(&self) -> RuntimeState {
        let registry = self.load_registry().unwrap_or_default();
        match registry.current() {
            Some(rt) => RuntimeState::Installed { current: rt.into() },
            None => RuntimeState::Missing,
        }
    }

    /// Resolve an installed runtime by profile target version, falling back to the current
    /// runtime for older profiles that predate explicit kernel selection.
    pub fn resolve_runtime(&self, target_version: &str) -> Option<InstalledRuntime> {
        let registry = self.load_registry().ok()?;
        let target_version = target_version.trim();
        if !target_version.is_empty() {
            if let Some(runtime) = registry
                .list()
                .iter()
                .find(|runtime| runtime.version == target_version)
            {
                return Some(runtime.clone());
            }
        }
        registry.current().cloned()
    }

    /// All installed runtimes in insertion order, the active version label, and the capability of
    /// that active version.
    pub fn installed_snapshot(&self) -> AppResult<InstalledRuntimesSnapshot> {
        let registry = self.load_registry()?;
        let installed: Vec<RuntimeVersionPayload> = registry
            .list()
            .iter()
            .map(RuntimeVersionPayload::from)
            .collect();
        Ok(InstalledRuntimesSnapshot {
            installed,
            current_version: registry.current_version.clone(),
            capability: self.capability_of(registry.current()),
        })
    }

    /// Describe an installed engine for the fingerprint surface. `None` — no engine selected — is
    /// *unknown*, not *stock*: Veil has nothing to read a capability off, so it may neither emit the
    /// patched vocabulary nor tell the user their engine lacks it.
    fn capability_of(&self, runtime: Option<&InstalledRuntime>) -> EngineCapability {
        match runtime {
            Some(runtime) => EngineCapability {
                engine_name: runtime
                    .engine_name
                    .clone()
                    .unwrap_or_else(|| self.manifest.name.clone()),
                flag_dialect: runtime.effective_dialect(),
                dialect_recorded: runtime.dialect_recorded(),
            },
            None => EngineCapability {
                engine_name: self.manifest.name.clone(),
                flag_dialect: FlagDialect::default(),
                dialect_recorded: false,
            },
        }
    }

    /// Fetch + filter releases. Maps every provider error to a stable user-facing code.
    pub fn list_available(&self) -> AppResult<Vec<AvailableRuntimePayload>> {
        match fetch_releases(&self.manifest) {
            Ok(releases) => {
                let candidates = select_candidates(&releases, &self.manifest);
                if candidates.is_empty() {
                    return Err(ErrorCode::RuntimeAssetNotFound.into());
                }
                Ok(candidates
                    .iter()
                    .map(AvailableRuntimePayload::from)
                    .collect())
            }
            Err(ProviderError::AssetNotFound) => Err(ErrorCode::RuntimeAssetNotFound.into()),
            Err(ProviderError::Network) => Err(ErrorCode::RuntimeNetworkUnavailable.into()),
            Err(ProviderError::Response(_)) => Err(ErrorCode::RuntimeGithubResponse.into()),
        }
    }

    /// Install a candidate. The pipeline runs synchronously on the caller's thread; the
    /// IPC layer wraps this in a `tauri::async_runtime::spawn_blocking` so the JS side stays
    /// responsive. `on_progress` is forwarded to the UI through a Tauri Channel.
    pub fn install<F: FnMut(RuntimeProgress)>(
        &self,
        candidate: &AvailableRuntimePayload,
        mut on_progress: F,
    ) -> AppResult<RuntimeVersionPayload> {
        let runtime_candidate = RuntimeCandidate {
            version: candidate.version.clone(),
            asset_name: candidate.asset_name.clone(),
            download_url: candidate.download_url.clone(),
            sha256: candidate.sha256.clone(),
            size_bytes: candidate.size_bytes,
            published_at: candidate.published_at.clone(),
        };
        self.install_candidate(candidate, runtime_candidate, &mut on_progress)
    }

    /// Install a candidate from a user-selected local ZIP. The candidate still supplies the
    /// trusted version and expected SHA-256; only the source URL is replaced.
    pub fn install_from_local_zip<F: FnMut(RuntimeProgress)>(
        &self,
        candidate: &AvailableRuntimePayload,
        local_zip_path: &str,
        mut on_progress: F,
    ) -> AppResult<RuntimeVersionPayload> {
        let local_zip = PathBuf::from(local_zip_path.trim());
        if local_zip.as_os_str().is_empty() || !local_zip.is_absolute() || !local_zip.is_file() {
            return Err(ErrorCode::RuntimeDiskWriteFailed.into());
        }
        let local_zip = local_zip
            .canonicalize()
            .map_err(|_| AppError::from(ErrorCode::RuntimeDiskWriteFailed))?;
        let runtime_candidate = RuntimeCandidate {
            version: candidate.version.clone(),
            asset_name: candidate.asset_name.clone(),
            download_url: local_zip.to_string_lossy().into_owned(),
            sha256: candidate.sha256.clone(),
            size_bytes: candidate.size_bytes,
            published_at: candidate.published_at.clone(),
        };
        self.install_candidate(candidate, runtime_candidate, &mut on_progress)
    }

    fn install_candidate<F: FnMut(RuntimeProgress)>(
        &self,
        candidate: &AvailableRuntimePayload,
        runtime_candidate: RuntimeCandidate,
        on_progress: &mut F,
    ) -> AppResult<RuntimeVersionPayload> {
        let registry = self.load_registry()?;
        if registry
            .list()
            .iter()
            .any(|r| r.version == candidate.version)
        {
            return Err(ErrorCode::RuntimeAlreadyInstalled.into());
        }
        let cancel = self.register_cancel(&candidate.version);
        let version_for_events = candidate.version.clone();
        let result = install_pipeline(
            &runtime_candidate,
            &self.install_root(),
            &self.manifest,
            cancel.clone(),
            |progress| {
                let mut event: RuntimeProgress = (&progress).into();
                if let RuntimeProgress::Completed { version } = &mut event {
                    *version = version_for_events.clone();
                }
                on_progress(event);
            },
        );
        self.clear_cancel(&candidate.version);

        match result {
            Ok(installed) => {
                let mut registry = self.load_registry().unwrap_or_default();
                let promote_to_current = registry.current_version.is_none();
                registry.upsert(installed.clone(), promote_to_current);
                self.save_registry(&registry)?;
                let payload: RuntimeVersionPayload = (&installed).into();
                on_progress(RuntimeProgress::Completed {
                    version: payload.version.clone(),
                });
                Ok(payload)
            }
            Err(PipelineError::Cancelled) => {
                on_progress(RuntimeProgress::Cancelled {
                    version: candidate.version.clone(),
                });
                Err(ErrorCode::RuntimeInstallCancelled.into())
            }
            Err(other) => {
                let code = map_pipeline_error(&other);
                on_progress(RuntimeProgress::Failed {
                    version: candidate.version.clone(),
                    code,
                });
                Err(code.into())
            }
        }
    }

    /// Flip the cancel flag for an in-flight install.
    pub fn cancel(&self, version: &str) {
        if let Some(token) = self.cancels.lock().expect("cancels lock").get(version) {
            token.store(true, Ordering::Release);
        }
    }

    /// Promote an already-installed runtime to "current".
    pub fn set_current(&self, version: &str) -> AppResult<RuntimeState> {
        let mut registry = self.load_registry()?;
        if !registry.list().iter().any(|r| r.version == version) {
            return Err(ErrorCode::RuntimeExecutableMissing.into());
        }
        registry.current_version = Some(version.to_string());
        self.save_registry(&registry)?;
        Ok(self.state())
    }

    /// Uninstall a runtime. Refuses to uninstall the currently selected version.
    pub fn uninstall(&self, version: &str) -> AppResult<()> {
        let mut registry = self.load_registry()?;
        if registry.current_version.as_deref() == Some(version) {
            return Err(ErrorCode::RuntimeCannotUninstallCurrent.into());
        }
        let removed = registry
            .remove(version)
            .ok_or_else(|| AppError::from(ErrorCode::RuntimeExecutableMissing))?;
        self.save_registry(&registry)?;
        // Best-effort directory cleanup.
        if let Some(dir) = removed.executable_path.parent() {
            let _ = remove_dir_if_under(&self.install_root(), dir);
        }
        Ok(())
    }

    fn register_cancel(&self, version: &str) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(false));
        self.cancels
            .lock()
            .expect("cancels lock")
            .insert(version.to_string(), token.clone());
        token
    }

    fn clear_cancel(&self, version: &str) {
        self.cancels.lock().expect("cancels lock").remove(version);
    }
}

fn map_pipeline_error(err: &PipelineError) -> ErrorCode {
    match err {
        PipelineError::Cancelled => ErrorCode::RuntimeInstallCancelled,
        PipelineError::ExecutableMissing => ErrorCode::RuntimeExecutableMissing,
        PipelineError::Install(InstallError::ChecksumMismatch) => {
            ErrorCode::RuntimeChecksumMismatch
        }
        PipelineError::Install(InstallError::Download) => ErrorCode::RuntimeDownloadFailed,
        PipelineError::Install(InstallError::UnsafeZipEntry) => ErrorCode::RuntimeExtractFailed,
        PipelineError::Install(InstallError::Io(_)) => ErrorCode::RuntimeDiskWriteFailed,
        PipelineError::Install(InstallError::Zip(_)) => ErrorCode::RuntimeExtractFailed,
        PipelineError::Io(_) => ErrorCode::RuntimeDiskWriteFailed,
        PipelineError::Zip(_) => ErrorCode::RuntimeExtractFailed,
    }
}

fn remove_dir_if_under(root: &Path, target: &Path) -> std::io::Result<()> {
    // Don't touch anything outside the install root.
    if !target.starts_with(root) {
        return Ok(());
    }
    if target.exists() {
        std::fs::remove_dir_all(target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::str::FromStr;
    use zip::write::SimpleFileOptions;

    #[test]
    fn install_root_path_defaults_to_versions_next_to_registry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("runtime").join("registry.json");
        let service = RuntimeService::new(registry_path);

        assert_eq!(
            service.install_root_path(),
            temp.path()
                .join("runtime")
                .join("versions")
                .to_string_lossy()
        );
    }

    /// After a move, the stored absolute `executable_path` points into a directory that no longer
    /// exists. Without the rebase the registry would list a version the launcher cannot start —
    /// the failure mode is a profile that reports "runtime executable missing" forever.
    #[test]
    fn rebases_executable_paths_onto_the_new_install_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let old_root = temp.path().join("old");
        let new_root = temp.path().join("new");
        let old_install_root = old_root.join("versions");
        let registry = RuntimeRegistryFile {
            current_version: Some("151.0.7922.137-1.1001".to_string()),
            installed: vec![InstalledRuntime {
                version: "151.0.7922.137-1.1001".to_string(),
                channel: "stable".to_string(),
                executable_path: old_install_root
                    .join("151.0.7922.137-1.1001")
                    .join("chrome.exe"),
                installed_at: "2026-08-23T10:31:51Z".to_string(),
                sha256: "ad35529a".to_string(),
                size_bytes: Some(205_007_900),
                flag_dialect: Some(FlagDialect::VeilPatched),
                engine_name: Some("Veil-Chromium".to_string()),
                github_owner: Some("Maishan-Inc".to_string()),
                github_repo: Some("Veil-Chromium".to_string()),
            }],
        };
        registry
            .save(&new_root.join("registry.json"))
            .expect("registry");

        let service = RuntimeService::new(new_root.join("registry.json"));
        service
            .rebase_installed_paths(&old_install_root)
            .expect("rebase");

        let reloaded = RuntimeRegistryFile::load(&new_root.join("registry.json")).expect("reload");
        assert_eq!(
            reloaded.installed[0].executable_path,
            new_root
                .join("versions")
                .join("151.0.7922.137-1.1001")
                .join("chrome.exe")
        );
    }

    /// A path that matches neither root is already broken; rebuilding it from the version keeps the
    /// registry internally consistent instead of leaving one dead entry behind.
    #[test]
    fn rebases_a_path_that_does_not_match_the_old_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let new_root = temp.path().join("new");
        let registry = RuntimeRegistryFile {
            current_version: None,
            installed: vec![InstalledRuntime {
                version: "v1".to_string(),
                channel: "stable".to_string(),
                executable_path: PathBuf::from("Z:/gone/v1/chrome.exe"),
                installed_at: "2026-08-23T10:31:51Z".to_string(),
                sha256: "abc".to_string(),
                size_bytes: None,
                flag_dialect: None,
                engine_name: None,
                github_owner: None,
                github_repo: None,
            }],
        };
        registry
            .save(&new_root.join("registry.json"))
            .expect("registry");

        let service = RuntimeService::new(new_root.join("registry.json"));
        service
            .rebase_installed_paths(&temp.path().join("old").join("versions"))
            .expect("rebase");

        let reloaded = RuntimeRegistryFile::load(&new_root.join("registry.json")).expect("reload");
        assert_eq!(
            reloaded.installed[0].executable_path,
            new_root.join("versions").join("v1").join("chrome.exe")
        );
    }

    #[test]
    fn installs_local_zip_and_promotes_first_runtime() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("runtime").join("registry.json");
        let zip_path = temp.path().join("engine.zip");
        write_test_zip(&zip_path);
        let sha256 = sha256_file(&zip_path);
        let size_bytes = zip_path.metadata().expect("metadata").len();
        let manifest = EngineManifest::from_str(
            r#"
            [engine]
            name = "TestEngine"
            github_owner = "example"
            github_repo = "engine"
            version_regex = "^v([0-9]+)$"
            asset_regex = "^engine\\.zip$"
            exe_path_in_zip = "chrome.exe"
            channel = "free"
            "#,
        )
        .expect("manifest");
        let service = RuntimeService::new(registry_path.clone()).with_manifest(manifest);
        let candidate = AvailableRuntimePayload {
            version: "120".to_string(),
            asset_name: "engine.zip".to_string(),
            download_url: "https://example.invalid/engine.zip".to_string(),
            sha256,
            size_bytes: Some(size_bytes),
            published_at: None,
        };
        let mut events = Vec::new();

        let installed = service
            .install_from_local_zip(&candidate, &zip_path.to_string_lossy(), |event| {
                events.push(event);
            })
            .expect("install");

        assert_eq!(installed.version, "120");
        assert!(Path::new(&installed.executable_path).exists());
        assert!(events
            .iter()
            .any(|event| matches!(event, RuntimeProgress::Verifying)));
        let registry = RuntimeRegistryFile::load(&registry_path).expect("registry");
        assert_eq!(registry.current_version.as_deref(), Some("120"));
        assert_eq!(registry.installed.len(), 1);
        assert_eq!(registry.installed[0].version, "120");
    }

    #[test]
    fn local_zip_install_rejects_empty_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("runtime").join("registry.json");
        let service = RuntimeService::new(registry_path);
        let candidate = AvailableRuntimePayload {
            version: "120".to_string(),
            asset_name: "engine.zip".to_string(),
            download_url: "https://example.invalid/engine.zip".to_string(),
            sha256: "a".repeat(64),
            size_bytes: None,
            published_at: None,
        };

        let err = service
            .install_from_local_zip(&candidate, " ", |_| {})
            .expect_err("empty path should fail");
        assert_eq!(err.code(), ErrorCode::RuntimeDiskWriteFailed);
    }

    #[test]
    fn resolves_target_runtime_and_falls_back_to_current() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("runtime").join("registry.json");
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(test_runtime("120"), true);
        registry.upsert(test_runtime("119"), false);
        registry.save(&registry_path).expect("registry");
        let service = RuntimeService::new(registry_path);

        assert_eq!(
            service.resolve_runtime("119").expect("target").version,
            "119"
        );
        assert_eq!(
            service
                .resolve_runtime("missing")
                .expect("fallback")
                .version,
            "120"
        );
    }

    /// The launch path's composition, minus the Tauri plumbing: every `commands::profiles` launch does
    /// `resolve_runtime(&profile.browser_version)` and then reads `effective_dialect()` off *that*
    /// entry. So a profile pinned to the stock engine must launch as stock even while the patched one is
    /// current, and an entry with nothing recorded must launch as stock even while the manifest says
    /// `veil-patched`. Before 5.5c all three of these answered `veil-patched`.
    #[test]
    fn the_dialect_travels_with_the_resolved_runtime() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("runtime").join("registry.json");
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(test_runtime("151.0.7922.137-1.1001"), true);
        registry.upsert(stock_runtime("151.0.7922.137-1.1"), false);
        registry.upsert(legacy_runtime("146.0.7680.177.5"), false);
        registry.save(&registry_path).expect("registry");
        let service = RuntimeService::new(registry_path);

        let dialect_of = |target: &str| {
            service
                .resolve_runtime(target)
                .expect("resolved")
                .effective_dialect()
        };
        assert_eq!(
            dialect_of("151.0.7922.137-1.1001"),
            FlagDialect::VeilPatched
        );
        assert_eq!(dialect_of("151.0.7922.137-1.1"), FlagDialect::StockChromium);
        assert_eq!(dialect_of("146.0.7680.177.5"), FlagDialect::StockChromium);
        // An unknown target falls back to `current`, and takes that engine's dialect with it.
        assert_eq!(dialect_of("nonexistent"), FlagDialect::VeilPatched);
    }

    fn write_test_zip(path: &Path) {
        let file = File::create(path).expect("zip file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("chrome.exe", SimpleFileOptions::default())
            .expect("start file");
        zip.write_all(b"fake executable").expect("write exe");
        zip.finish().expect("finish zip");
    }

    fn sha256_file(path: &Path) -> String {
        let mut file = File::open(path).expect("open");
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 4096];
        loop {
            let n = file.read(&mut buffer).expect("read");
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        format!("{:x}", hasher.finalize())
    }

    fn test_runtime(version: &str) -> InstalledRuntime {
        InstalledRuntime {
            version: version.to_string(),
            channel: "free".to_string(),
            executable_path: PathBuf::from(format!("C:/Veil/runtime/{version}/chrome.exe")),
            installed_at: "2026-06-30T00:00:00Z".to_string(),
            sha256: "a".repeat(64),
            size_bytes: Some(42),
            flag_dialect: Some(FlagDialect::VeilPatched),
            engine_name: Some("Veil-Chromium".to_string()),
            github_owner: Some("Maishan-Inc".to_string()),
            github_repo: Some("Veil-Chromium".to_string()),
        }
    }

    /// An entry with a recorded dialect that is deliberately *not* the manifest's.
    fn stock_runtime(version: &str) -> InstalledRuntime {
        InstalledRuntime {
            flag_dialect: Some(FlagDialect::StockChromium),
            engine_name: Some("ungoogled-chromium".to_string()),
            github_owner: Some("ungoogled-software".to_string()),
            github_repo: Some("ungoogled-chromium-windows".to_string()),
            ..test_runtime(version)
        }
    }

    /// The shape every registry written before 2026-08-26 has: no identity at all.
    fn legacy_runtime(version: &str) -> InstalledRuntime {
        InstalledRuntime {
            flag_dialect: None,
            engine_name: None,
            github_owner: None,
            github_repo: None,
            ..test_runtime(version)
        }
    }

    fn snapshot_for(runtimes: Vec<InstalledRuntime>, current: Option<&str>) -> EngineCapability {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("runtime").join("registry.json");
        let mut registry = RuntimeRegistryFile::default();
        for runtime in runtimes {
            registry.upsert(runtime, false);
        }
        registry.current_version = current.map(str::to_string);
        registry.save(&registry_path).expect("registry");
        RuntimeService::new(registry_path)
            .installed_snapshot()
            .expect("snapshot")
            .capability
    }

    /// Two engines with different dialects, one selection. The reported capability belongs to the
    /// entry `current_version` names — not to the manifest, and not to the first installed entry.
    /// Before 5.5c this was `manifest.flag_dialect`, so both answers were `veil-patched`.
    #[test]
    fn the_snapshot_reports_the_capability_of_the_selected_engine() {
        let entries = vec![
            stock_runtime("151.0.7922.137-1.1"),
            test_runtime("151.0.7922.137-1.1001"),
        ];

        let stock = snapshot_for(entries.clone(), Some("151.0.7922.137-1.1"));
        assert_eq!(stock.flag_dialect, FlagDialect::StockChromium);
        assert!(stock.dialect_recorded, "the entry recorded it");
        assert_eq!(stock.engine_name, "ungoogled-chromium");

        let patched = snapshot_for(entries, Some("151.0.7922.137-1.1001"));
        assert_eq!(patched.flag_dialect, FlagDialect::VeilPatched);
        assert!(patched.dialect_recorded);
        assert_eq!(patched.engine_name, "Veil-Chromium");
    }

    /// An engine installed before Veil recorded capability behaves as stock and is reported as
    /// *unrecorded*, so the UI can degrade without claiming the engine was measured.
    #[test]
    fn a_legacy_entry_is_stock_but_not_a_recorded_stock() {
        let capability = snapshot_for(
            vec![legacy_runtime("146.0.7680.177.5")],
            Some("146.0.7680.177.5"),
        );
        assert_eq!(capability.flag_dialect, FlagDialect::StockChromium);
        assert!(!capability.dialect_recorded);
        // Nothing was recorded, so the only name available is the manifest's.
        assert_eq!(capability.engine_name, EngineManifest::default().name);
    }

    /// No selection is *unknown*, not *stock*: there is nothing to read a capability off, so the
    /// patched vocabulary must not be emitted and the banner must not claim a measurement.
    #[test]
    fn no_selected_engine_reports_the_conservative_default() {
        let capability = snapshot_for(Vec::new(), None);
        assert_eq!(capability.flag_dialect, FlagDialect::StockChromium);
        assert!(!capability.dialect_recorded);
    }
}
