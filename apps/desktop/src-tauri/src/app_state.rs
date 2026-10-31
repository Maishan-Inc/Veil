use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use veil_bridge::EngineProcess;
use veil_core::{AppError, AppResult, ErrorCode};
use veil_database::Database;
use veil_secure_storage::{VaultKeyError, VaultKeystore, VaultWrapperKind};

use crate::engine_root;
use crate::local_api::SensitiveTokenStore;
use crate::migrate;
use crate::paths::VeilPaths;
use crate::services::{
    AiService, DiagnosticsService, ExtensionService, LocalApiService, OnboardingService,
    ProfileService, ProxyService, RuntimeService,
};

/// Shared, Tauri-managed application state.
///
/// Phase 1B persisted profiles to SQLCipher and sealed the DB key via DPAPI. Phase 2
/// brings up the proxy library on top of the same database connection; both services
/// share an `Arc<Mutex<Database>>` so reads/writes serialize on the same mutex without
/// opening a second SQLCipher connection (which would double-decrypt for no benefit).
///
/// Locking strategy:
/// - `profiles` and `proxies` share the inner DB mutex. Operations are short-lived so
///   contention stays trivial for the workload (thousands of rows, mostly cached).
/// - `runtime` already serializes its own state (cancel-token map, registry file).
/// - `launched` is a separate mutex so a long-running profile launch never blocks the DB.
pub struct AppState {
    /// Root for all Veil data: keyfile, database, per-profile user-data-dirs.
    pub data_dir: PathBuf,
    /// The OS-default data directory, which is where the `paths.json` pointer file lives. It is
    /// the one location that cannot move, because it records where everything else went.
    pub default_data_dir: PathBuf,
    /// Local-only diagnostic recorder. Independent of the database on purpose: the moments a
    /// diagnostic matters most are when the encrypted store is unavailable.
    pub diagnostics: DiagnosticsService,
    /// User-chosen locations, as persisted. Behind a lock because Settings can rewrite them.
    pub paths: Mutex<VeilPaths>,
    pub runtime: RuntimeService,
    /// Where engine runtimes are installed, and which rule decided that. Changes when the user
    /// moves the engine, so it is read through [`AppState::engine_root`].
    engine_root: Mutex<engine_root::EngineRoot>,
    pub onboarding: OnboardingService,
    vault: VaultKeystore,
    database: Mutex<Option<DatabaseServices>>,
    /// Live engine processes keyed by `profile_id`.
    pub launched: Mutex<HashMap<String, EngineProcess>>,
    /// Temporary engine processes keyed by one-time `session_id`.
    pub temporary_launched: Mutex<HashMap<String, TemporaryEngineProcess>>,
    pub(crate) sensitive_tokens: SensitiveTokenStore,
}

/// A running temporary browser. The user-data directory is deleted when the process
/// is closed or reaped.
pub struct TemporaryEngineProcess {
    pub process: EngineProcess,
    pub data_dir: PathBuf,
}

#[derive(Clone)]
struct DatabaseServices {
    ai: AiService,
    extensions: ExtensionService,
    local_api: LocalApiService,
    profiles: ProfileService,
    proxies: ProxyService,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub locked: bool,
    pub password_required: bool,
    pub wrapper: String,
}

#[derive(Debug)]
pub enum AppStateInitError {
    DataDir,
    Key(ErrorCode),
    Database(ErrorCode),
}

impl std::fmt::Display for AppStateInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppStateInitError::DataDir => write!(f, "could not create data dir"),
            AppStateInitError::Key(code) => write!(f, "keystore failed ({code})"),
            AppStateInitError::Database(code) => write!(f, "database failed ({code})"),
        }
    }
}

impl AppState {
    /// Open (and create if needed) the keystore + database, returning a ready `AppState`.
    ///
    /// `default_data_dir` is the OS-default location. The *effective* data directory is read from
    /// the `paths.json` pointer file inside it, so a user who moved their data gets the moved
    /// location and everyone else gets the default.
    ///
    /// Errors here are fatal — the app's main entry point panics with a stable error code
    /// rather than running with a degraded silent fallback.
    pub fn open(default_data_dir: PathBuf) -> Result<Self, AppStateInitError> {
        let mut paths = VeilPaths::load(&default_data_dir);
        let mut data_dir = paths
            .data_dir
            .clone()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| default_data_dir.clone());

        // A data-directory move requested during the last session, performed now — before the
        // keystore and the database are opened, which is the only moment nothing holds a handle on
        // them. A failed move leaves everything where it was and clears the request rather than
        // retrying on every launch, because a target that cannot be written to will not fix itself.
        if let Some(pending) = paths.pending_data_dir.take() {
            if migrate::move_directory(&data_dir, &pending).is_ok() {
                // A configured engine root *inside* the data directory travelled with it, so the
                // recorded path now points at a directory that no longer exists. Left alone, the
                // app would start up reporting no engine installed and the user would have no way
                // to connect that to the move they just made.
                if let Some(engine_root) = paths.engine_root.clone() {
                    if let Ok(relative) = engine_root.strip_prefix(&data_dir) {
                        paths.engine_root = Some(pending.join(relative));
                    }
                }
                data_dir = pending.clone();
                paths.data_dir = Some(pending);
            }
            // `paths.json` itself lives in the default directory and was moved along with
            // everything else; writing it back restores the one file that must not travel.
            let _ = paths.save(&default_data_dir);
        }
        std::fs::create_dir_all(&data_dir).map_err(|_| AppStateInitError::DataDir)?;

        let diagnostics = DiagnosticsService::new(&data_dir);
        let vault = VaultKeystore::new(&data_dir);
        let database = match vault.load_or_create_dpapi() {
            Ok(key) => Some(open_database_services(&data_dir, &key)?),
            Err(VaultKeyError::PasswordRequired) => None,
            Err(err) => return Err(AppStateInitError::Key(map_vault_key_error(err))),
        };

        // Resolved before the registry is touched, because the answer decides which
        // `registry.json` the app is even reading. See `engine_root::resolve_engine_root`.
        let engine_root = engine_root::resolve_engine_root(
            &data_dir,
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .as_deref(),
            env::var("VEIL_RUNTIME_ROOT").ok(),
            paths.engine_root.clone(),
            &engine_root::probe_writable,
            &engine_root::has_registry,
        );
        let registry_path = engine_root.path.join("registry.json");
        Ok(Self {
            onboarding: OnboardingService::new(&data_dir),
            data_dir,
            default_data_dir,
            diagnostics,
            paths: Mutex::new(paths),
            vault,
            database: Mutex::new(database),
            runtime: RuntimeService::new(registry_path),
            engine_root: Mutex::new(engine_root),
            launched: Mutex::new(HashMap::new()),
            temporary_launched: Mutex::new(HashMap::new()),
            sensitive_tokens: SensitiveTokenStore::default(),
        })
    }

    /// Clone of the diagnostic recorder for command-layer outcome logging.
    pub fn diagnostics(&self) -> DiagnosticsService {
        self.diagnostics.clone()
    }

    /// Current engine root and the rule that chose it.
    pub fn engine_root(&self) -> engine_root::EngineRoot {
        self.engine_root
            .lock()
            .expect("engine root lock poisoned")
            .clone()
    }

    /// Record a data-directory move for the next startup, after checking it can succeed.
    ///
    /// Nothing is moved here. The database, the keystore and every per-profile user-data directory
    /// are open while the app runs, and Windows will not move a file that has a handle on it —
    /// [`VeilPaths::pending_data_dir`] explains why doing it at startup is the only ordering where
    /// a failure costs nothing. The caller is expected to restart the app.
    pub fn request_data_dir_move(&self, new_dir: PathBuf) -> AppResult<()> {
        if new_dir.as_os_str().is_empty() {
            return Err(ErrorCode::MigrationFailed.into());
        }
        if !self
            .launched
            .lock()
            .expect("launched lock poisoned")
            .is_empty()
            || !self
                .temporary_launched
                .lock()
                .expect("temporary launched lock poisoned")
                .is_empty()
        {
            return Err(ErrorCode::MigrationBusy.into());
        }
        migrate::validate_target(&self.data_dir, &new_dir).map_err(|err| match err {
            migrate::MigrationError::TargetNotEmpty => ErrorCode::MigrationTargetNotEmpty,
            migrate::MigrationError::NestedPaths => ErrorCode::MigrationNestedPaths,
            migrate::MigrationError::Io => ErrorCode::MigrationFailed,
        })?;

        let mut paths = self.paths.lock().expect("paths lock poisoned");
        paths.pending_data_dir = Some(new_dir);
        paths
            .save(&self.default_data_dir)
            .map_err(|_| AppError::from(ErrorCode::MigrationFailed))
    }

    /// Drop a recorded data-directory move. Nothing has been moved yet, so this only clears intent.
    pub fn cancel_data_dir_move(&self) -> AppResult<()> {
        let mut paths = self.paths.lock().expect("paths lock poisoned");
        paths.pending_data_dir = None;
        paths
            .save(&self.default_data_dir)
            .map_err(|_| AppError::from(ErrorCode::MigrationFailed))
    }

    /// Move the installed engines to `new_root`, then remember it as the user's choice.
    ///
    /// Order matters and is not interchangeable: the files move first, then the registry is
    /// re-pointed and its absolute paths rebased, and only then is the choice persisted. A failure
    /// at any step leaves the previous location working — the alternative, recording the new path
    /// before the move succeeds, would produce an app that starts up pointing at an empty folder.
    pub fn move_engine_root(&self, new_root: PathBuf) -> AppResult<engine_root::EngineRoot> {
        if new_root.as_os_str().is_empty() {
            return Err(ErrorCode::MigrationFailed.into());
        }
        // A running `chrome.exe` holds its own image open, and Windows refuses to rename a
        // directory containing an open file. Failing here beats failing halfway through a copy.
        let engines_running = !self
            .launched
            .lock()
            .expect("launched lock poisoned")
            .is_empty()
            || !self
                .temporary_launched
                .lock()
                .expect("temporary launched lock poisoned")
                .is_empty();
        if engines_running {
            return Err(ErrorCode::MigrationBusy.into());
        }

        let old_root = self.engine_root().path;
        let old_install_root = self.runtime.install_root();
        migrate::move_directory(&old_root, &new_root).map_err(|err| match err {
            migrate::MigrationError::TargetNotEmpty => ErrorCode::MigrationTargetNotEmpty,
            migrate::MigrationError::NestedPaths => ErrorCode::MigrationNestedPaths,
            migrate::MigrationError::Io => ErrorCode::MigrationFailed,
        })?;

        self.runtime
            .set_registry_path(new_root.join("registry.json"));
        self.runtime.rebase_installed_paths(&old_install_root)?;

        let mut paths = self.paths.lock().expect("paths lock poisoned");
        paths.engine_root = Some(new_root.clone());
        paths
            .save(&self.default_data_dir)
            .map_err(|_| AppError::from(ErrorCode::MigrationFailed))?;

        let resolved = engine_root::EngineRoot {
            path: new_root,
            source: engine_root::EngineRootSource::UserSetting,
        };
        *self.engine_root.lock().expect("engine root lock poisoned") = resolved.clone();
        Ok(resolved)
    }

    pub fn vault_status(&self) -> VaultStatus {
        let unlocked = self
            .database
            .lock()
            .expect("database lock poisoned")
            .is_some();
        let wrapper = self
            .vault
            .active_wrapper()
            .unwrap_or(VaultWrapperKind::Dpapi);
        VaultStatus {
            locked: !unlocked,
            password_required: wrapper == VaultWrapperKind::Password,
            wrapper: wrapper_name(wrapper).to_string(),
        }
    }

    pub fn profiles(&self) -> AppResult<ProfileService> {
        self.database
            .lock()
            .expect("database lock poisoned")
            .as_ref()
            .map(|services| services.profiles.clone())
            .ok_or_else(|| AppError::from(ErrorCode::StorageLocked))
    }

    pub fn ai(&self) -> AppResult<AiService> {
        self.database
            .lock()
            .expect("database lock poisoned")
            .as_ref()
            .map(|services| services.ai.clone())
            .ok_or_else(|| AppError::from(ErrorCode::StorageLocked))
    }

    pub fn extensions(&self) -> AppResult<ExtensionService> {
        self.database
            .lock()
            .expect("database lock poisoned")
            .as_ref()
            .map(|services| services.extensions.clone())
            .ok_or_else(|| AppError::from(ErrorCode::StorageLocked))
    }

    pub fn local_api(&self) -> AppResult<LocalApiService> {
        self.database
            .lock()
            .expect("database lock poisoned")
            .as_ref()
            .map(|services| services.local_api.clone())
            .ok_or_else(|| AppError::from(ErrorCode::StorageLocked))
    }

    pub fn proxies(&self) -> AppResult<ProxyService> {
        self.database
            .lock()
            .expect("database lock poisoned")
            .as_ref()
            .map(|services| services.proxies.clone())
            .ok_or_else(|| AppError::from(ErrorCode::StorageLocked))
    }

    pub fn set_unlocked_database(
        &self,
        services: (
            AiService,
            ExtensionService,
            LocalApiService,
            ProfileService,
            ProxyService,
        ),
    ) {
        let mut database = self.database.lock().expect("database lock poisoned");
        *database = Some(DatabaseServices {
            ai: services.0,
            extensions: services.1,
            local_api: services.2,
            profiles: services.3,
            proxies: services.4,
        });
    }
}

fn open_database_services(
    data_dir: &std::path::Path,
    key: &str,
) -> Result<DatabaseServices, AppStateInitError> {
    let db_path = data_dir.join("veil.db");
    let database = Database::open(&db_path, key)
        .map_err(|_| AppStateInitError::Database(ErrorCode::StorageDatabaseReadFailed))?;
    let db = Arc::new(Mutex::new(database));
    Ok(DatabaseServices {
        ai: AiService::new(db.clone()),
        extensions: ExtensionService::new(db.clone(), data_dir.join("extensions")),
        local_api: LocalApiService::new(db.clone()),
        profiles: ProfileService::new(db.clone()),
        proxies: ProxyService::new(db),
    })
}

pub(crate) fn open_database_service_pair(
    data_dir: &std::path::Path,
    key: &str,
) -> Result<
    (
        AiService,
        ExtensionService,
        LocalApiService,
        ProfileService,
        ProxyService,
    ),
    AppStateInitError,
> {
    let services = open_database_services(data_dir, key)?;
    Ok((
        services.ai,
        services.extensions,
        services.local_api,
        services.profiles,
        services.proxies,
    ))
}

pub(crate) fn map_vault_key_error(err: VaultKeyError) -> ErrorCode {
    match err {
        VaultKeyError::Read
        | VaultKeyError::Unseal
        | VaultKeyError::Corrupted
        | VaultKeyError::PasswordRequired => ErrorCode::StorageDecryptionFailed,
        VaultKeyError::VerificationFailed => ErrorCode::SecurityVerificationFailed,
        VaultKeyError::Write | VaultKeyError::Seal => ErrorCode::StorageEncryptionFailed,
    }
}

fn wrapper_name(wrapper: VaultWrapperKind) -> &'static str {
    match wrapper {
        VaultWrapperKind::Dpapi => "dpapi",
        VaultWrapperKind::Password => "password",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine root moves and nothing else: the database and keystore are resolved from
    /// `data_dir`, so a redirected engine root must not drag them along. The resolution rules
    /// themselves are tested in `engine_root`.
    #[test]
    fn a_redirected_engine_root_does_not_move_the_data_dir() {
        let data_dir = PathBuf::from("C:/data/Veil");
        let resolved = engine_root::resolve_engine_root(
            &data_dir,
            None,
            Some("D:/engines".to_string()),
            None,
            &|_| true,
            &|_| false,
        );
        assert!(!resolved.path.starts_with(&data_dir));
        assert_eq!(
            data_dir.join("veil.db"),
            PathBuf::from("C:/data/Veil/veil.db")
        );
    }
}
