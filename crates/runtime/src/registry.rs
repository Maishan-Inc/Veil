use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::FlagDialect;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// One engine installation, as recorded on disk.
///
/// The last four fields answer "**which** engine is this" and were added 2026-08-26; every entry
/// written before then has none of them, which is why all four are `Option` with `#[serde(default)]`
/// rather than defaulted values. Absence is a real state and it is not the same state as
/// `stock-chromium`: an engine installed before Veil recorded capability is *unproven*, not *proven
/// plain*. [`Self::effective_dialect`] collapses the two for the purpose of emitting switches — the
/// only safe direction — while the UI keeps them apart, because a banner that says "this engine does
/// not support fingerprint spoofing" about an engine nobody measured is stating something untested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledRuntime {
    pub version: String,
    /// Manifest `channel` label. **Not** engine identity: it says `stable` for every engine any
    /// manifest has ever pointed at, so it can distinguish neither repository nor dialect.
    pub channel: String,
    pub executable_path: PathBuf,
    pub installed_at: String,
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Switch vocabulary **this** engine implements, recorded at install time from the manifest that
    /// installed it. `None` = installed before Veil recorded it; see [`Self::effective_dialect`].
    #[serde(default)]
    pub flag_dialect: Option<FlagDialect>,
    /// `engine.name` from the manifest that installed this.
    #[serde(default)]
    pub engine_name: Option<String>,
    /// `engine.github_owner` / `engine.github_repo` from that manifest, so "where did this engine
    /// come from" is answerable from the registry instead of guessed from a version number.
    #[serde(default)]
    pub github_owner: Option<String>,
    #[serde(default)]
    pub github_repo: Option<String>,
}

impl InstalledRuntime {
    /// The dialect `to_engine_args` must be gated on for a launch of **this** engine.
    ///
    /// An entry with no recorded dialect resolves to [`FlagDialect::StockChromium`]: it cannot be
    /// proven patched, and Chromium ignores unknown switches silently, so emitting the patched
    /// vocabulary at it would make Veil report a profile as spoofed when nothing applied it. The
    /// asymmetry is deliberate — defaulting the other way would freeze today's bug into the on-disk
    /// format.
    pub fn effective_dialect(&self) -> FlagDialect {
        self.flag_dialect.unwrap_or_default()
    }

    /// True when the dialect was read off this entry rather than defaulted. The fingerprint surface
    /// needs this to tell "known stock" from "unknown", which are the same *behaviour* and different
    /// *claims*.
    pub fn dialect_recorded(&self) -> bool {
        self.flag_dialect.is_some()
    }
}

/// On-disk registry of installed engine runtimes and the selected current version.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeRegistryFile {
    pub current_version: Option<String>,
    pub installed: Vec<InstalledRuntime>,
}

impl RuntimeRegistryFile {
    /// The currently selected runtime, if any.
    pub fn current(&self) -> Option<&InstalledRuntime> {
        let version = self.current_version.as_ref()?;
        self.installed
            .iter()
            .find(|runtime| &runtime.version == version)
    }

    /// Load from disk; a missing file is treated as an empty registry (no network, no error).
    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(RegistryError::Io(err)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), RegistryError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// Insert or replace a runtime, optionally promoting it to the current version.
    pub fn upsert(&mut self, runtime: InstalledRuntime, make_current: bool) {
        match self
            .installed
            .iter_mut()
            .find(|r| r.version == runtime.version)
        {
            Some(existing) => *existing = runtime.clone(),
            None => self.installed.push(runtime.clone()),
        }
        if make_current {
            self.current_version = Some(runtime.version);
        }
    }

    /// Remove an installed runtime by version. If it was the current version, clear the
    /// selection. Returns the removed entry if any.
    pub fn remove(&mut self, version: &str) -> Option<InstalledRuntime> {
        let index = self.installed.iter().position(|r| r.version == version)?;
        let removed = self.installed.remove(index);
        if self.current_version.as_deref() == Some(version) {
            self.current_version = None;
        }
        Some(removed)
    }

    /// Iterate installed runtimes, newest installation timestamps last (insertion order).
    pub fn list(&self) -> &[InstalledRuntime] {
        &self.installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(version: &str) -> InstalledRuntime {
        InstalledRuntime {
            version: version.to_string(),
            channel: "free".to_string(),
            executable_path: PathBuf::from(format!("C:/Veil/runtime/{version}/engine.exe")),
            installed_at: "2026-06-27T00:00:00Z".to_string(),
            sha256: "a".repeat(64),
            size_bytes: Some(150_000_000),
            flag_dialect: Some(FlagDialect::VeilPatched),
            engine_name: Some("Veil-Chromium".to_string()),
            github_owner: Some("Maishan-Inc".to_string()),
            github_repo: Some("Veil-Chromium".to_string()),
        }
    }

    #[test]
    fn resolves_current_runtime() {
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(runtime("v120"), true);
        assert_eq!(registry.current().expect("current").version, "v120");
    }

    #[test]
    fn upsert_replaces_existing_version() {
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(runtime("v120"), true);
        registry.upsert(runtime("v120"), true);
        assert_eq!(registry.installed.len(), 1);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(runtime("v120"), true);
        registry.save(&path).unwrap();

        let loaded = RuntimeRegistryFile::load(&path).unwrap();
        assert_eq!(loaded, registry);
        assert_eq!(loaded.current().unwrap().version, "v120");
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = RuntimeRegistryFile::load(&dir.path().join("nope.json")).unwrap();
        assert_eq!(loaded, RuntimeRegistryFile::default());
    }

    #[test]
    fn remove_clears_current_when_target_is_active() {
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(runtime("v120"), true);
        registry.upsert(runtime("v119"), false);
        assert_eq!(registry.current().unwrap().version, "v120");
        let removed = registry.remove("v120").expect("removed");
        assert_eq!(removed.version, "v120");
        assert!(registry.current().is_none());
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn remove_keeps_current_when_target_is_inactive() {
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(runtime("v120"), true);
        registry.upsert(runtime("v119"), false);
        registry.remove("v119").expect("removed");
        assert_eq!(registry.current().unwrap().version, "v120");
    }

    /// The compatibility case that decides whether this fix is safe. Every entry written before
    /// 2026-08-26 lacks the four identity fields; such an entry must resolve to `stock-chromium`,
    /// because Veil cannot prove it is patched. Defaulting the other way would hand five switches no
    /// engine implements to exactly the installs the bug was found on.
    #[test]
    fn an_entry_without_a_recorded_dialect_loads_and_resolves_to_stock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        // Verbatim shape of a pre-2026-08-26 registry, including the trailing-field order.
        fs::write(
            &path,
            br#"{
              "current_version": "146.0.7680.177.5",
              "installed": [
                {
                  "version": "146.0.7680.177.5",
                  "channel": "free",
                  "executable_path": "C:\\Veil\\runtime\\146\\chrome.exe",
                  "installed_at": "2026-07-04T00:00:00Z",
                  "sha256": "deadbeef",
                  "size_bytes": 537000000
                }
              ]
            }"#,
        )
        .unwrap();

        let loaded = RuntimeRegistryFile::load(&path).expect("legacy registry must still load");
        let current = loaded.current().expect("current");
        assert_eq!(current.flag_dialect, None, "nothing was recorded");
        assert!(!current.dialect_recorded());
        assert_eq!(current.effective_dialect(), FlagDialect::StockChromium);
        assert_eq!(current.engine_name, None);
        assert_eq!(current.github_owner, None);
        assert_eq!(current.github_repo, None);
    }

    /// Two engines, two dialects, one selection: the dialect belongs to the entry `current_version`
    /// names, never to the registry as a whole and never to the manifest.
    #[test]
    fn each_entry_carries_its_own_dialect() {
        let mut registry = RuntimeRegistryFile::default();
        let mut stock = runtime("151.0.7922.137-1.1");
        stock.flag_dialect = Some(FlagDialect::StockChromium);
        stock.engine_name = Some("ungoogled-chromium".to_string());
        stock.github_owner = Some("ungoogled-software".to_string());
        registry.upsert(stock, true);
        registry.upsert(runtime("151.0.7922.137-1.1001"), false);

        assert_eq!(
            registry.current().unwrap().effective_dialect(),
            FlagDialect::StockChromium
        );
        registry.current_version = Some("151.0.7922.137-1.1001".to_string());
        assert_eq!(
            registry.current().unwrap().effective_dialect(),
            FlagDialect::VeilPatched
        );
    }

    /// The identity fields survive a save/load round trip, which is what makes "where did this engine
    /// come from" answerable after a restart rather than only at install time.
    #[test]
    fn identity_fields_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let mut registry = RuntimeRegistryFile::default();
        registry.upsert(runtime("151.0.7922.137-1.1001"), true);
        registry.save(&path).unwrap();

        let loaded = RuntimeRegistryFile::load(&path).unwrap();
        let current = loaded.current().expect("current");
        assert_eq!(current.flag_dialect, Some(FlagDialect::VeilPatched));
        assert_eq!(current.engine_name.as_deref(), Some("Veil-Chromium"));
        assert_eq!(current.github_owner.as_deref(), Some("Maishan-Inc"));
        assert_eq!(current.github_repo.as_deref(), Some("Veil-Chromium"));
    }
}
