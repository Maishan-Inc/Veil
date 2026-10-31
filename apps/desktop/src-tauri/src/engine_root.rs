use std::fs;
use std::path::{Path, PathBuf};

/// Which rule decided where the engine lives. Surfaced so the UI can explain the path instead of
/// presenting it as a bare string the user cannot account for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineRootSource {
    /// `VEIL_RUNTIME_ROOT` was set.
    EnvOverride,
    /// The user picked a directory in Settings.
    UserSetting,
    /// Default for an installed app: `<install dir>\runtime`.
    NextToApp,
    /// An engine was already installed at the old `<data_dir>\runtime` location, so Veil keeps
    /// using it instead of pretending nothing is installed. The user can move it from Settings.
    LegacyAdopted,
    /// Running from a Cargo build directory, so the engine goes to the data directory instead of
    /// somewhere `cargo clean` deletes. Only ever seen by developers.
    DevBuild,
    /// Fallback when the install directory is not writable (e.g. `C:\Program Files` without
    /// elevation).
    DataDir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRoot {
    pub path: PathBuf,
    pub source: EngineRootSource,
}

/// Resolve where engine runtimes are installed, in priority order:
///
/// 1. `VEIL_RUNTIME_ROOT` — the escape hatch. Highest, because it is how the detection-baseline
///    harness and the integration tests pin a root, and because the engine's *location* can decide
///    whether it runs at all (`ERROR_SXS_CANT_GEN_ACTCTX` under the user profile on some hosts).
/// 2. The directory the user chose in Settings.
/// 3. `<install dir>\runtime` — next to the app, which is what a user means by "install the engine
///    with the program". Skipped when it is not writable: an MSI/NSIS install lands in
///    `C:\Program Files\Veil`, and a 205 MB download into an unwritable directory must fail at
///    resolution time rather than four minutes into the transfer. Also skipped when an engine is
///    already installed at the historical location — moving 205 MB during startup would freeze a
///    splash screen for tens of seconds, so the existing install is adopted where it is and
///    Settings offers to move it.
/// 4. `<data_dir>\runtime` — the historical location, and the fallback that always works.
///
/// **Only the engine moves.** The keystore, the SQLCipher database and per-profile user-data
/// directories stay under `data_dir`; relocating the DPAPI-wrapped key would strand it.
pub fn resolve_engine_root(
    data_dir: &Path,
    exe_dir: Option<&Path>,
    env_override: Option<String>,
    configured: Option<PathBuf>,
    is_writable: &dyn Fn(&Path) -> bool,
    has_registry: &dyn Fn(&Path) -> bool,
) -> EngineRoot {
    // An empty or whitespace-only value means "unset", not "install at the filesystem root".
    if let Some(root) = env_override.filter(|value| !value.trim().is_empty()) {
        return EngineRoot {
            path: PathBuf::from(root.trim()),
            source: EngineRootSource::EnvOverride,
        };
    }
    if let Some(root) = configured.filter(|path| !path.as_os_str().is_empty()) {
        return EngineRoot {
            path: root,
            source: EngineRootSource::UserSetting,
        };
    }
    let legacy = data_dir.join("runtime");
    // A development run must not put a 205 MB engine somewhere `cargo clean` deletes. Reported as
    // its own reason rather than folded into the unwritable-directory fallback, because Settings
    // would otherwise tell a developer their program folder is read-only, which is not true.
    if exe_dir.is_some_and(is_cargo_target_dir) {
        return EngineRoot {
            path: legacy,
            source: EngineRootSource::DevBuild,
        };
    }
    if let Some(candidate) = exe_dir
        .map(|dir| dir.join("runtime"))
        .filter(|candidate| is_writable(candidate))
    {
        // Nothing next to the app yet, but something at the old location: use what exists.
        if !has_registry(&candidate) && has_registry(&legacy) {
            return EngineRoot {
                path: legacy,
                source: EngineRootSource::LegacyAdopted,
            };
        }
        return EngineRoot {
            path: candidate,
            source: EngineRootSource::NextToApp,
        };
    }
    EngineRoot {
        path: legacy,
        source: EngineRootSource::DataDir,
    }
}

/// True when `root` holds a runtime registry, i.e. an engine has been installed there.
pub fn has_registry(root: &Path) -> bool {
    root.join("registry.json").is_file()
}

/// True when the executable is running out of a Cargo build directory (`…/target/debug`,
/// `…/target/release`, or a `--target <triple>` variant).
///
/// Matched on the path rather than on `cfg!(debug_assertions)`, because behaving differently in
/// debug and release builds would mean the shipped path is the one never exercised.
fn is_cargo_target_dir(exe_dir: &Path) -> bool {
    let mut components = exe_dir.components().rev();
    let Some(last) = components.next() else {
        return false;
    };
    let profile_like = matches!(last.as_os_str().to_str(), Some("debug") | Some("release"));
    if !profile_like {
        return false;
    }
    // `target/debug` directly, or `target/<triple>/debug` for a cross build.
    components
        .take(2)
        .any(|component| component.as_os_str() == "target")
}

/// Probe a candidate root by actually creating it and writing a file, because the Windows answer
/// to "can I write here" is not derivable from metadata: `C:\Program Files` is readable, reports
/// no read-only flag, and still refuses a write without elevation.
pub fn probe_writable(candidate: &Path) -> bool {
    if fs::create_dir_all(candidate).is_err() {
        return false;
    }
    let probe = candidate.join(".veil-write-probe");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_writable(_: &Path) -> bool {
        true
    }
    fn never_writable(_: &Path) -> bool {
        false
    }
    fn no_registry_anywhere(_: &Path) -> bool {
        false
    }

    #[test]
    fn env_override_wins_over_everything() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Program Files/Veil")),
            Some("  H:/engines  ".to_string()),
            Some(PathBuf::from("E:/configured")),
            &always_writable,
            &no_registry_anywhere,
        );
        assert_eq!(root.path, PathBuf::from("H:/engines"));
        assert_eq!(root.source, EngineRootSource::EnvOverride);
    }

    /// `set VEIL_RUNTIME_ROOT=` leaves an empty string in the environment on Windows. Treating it
    /// as a path would install the engine at the filesystem root.
    #[test]
    fn blank_env_override_is_ignored() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            None,
            Some("   ".to_string()),
            None,
            &always_writable,
            &no_registry_anywhere,
        );
        assert_eq!(root.path, PathBuf::from("D:/data/runtime"));
        assert_eq!(root.source, EngineRootSource::DataDir);
    }

    #[test]
    fn user_setting_beats_the_install_directory() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Program Files/Veil")),
            None,
            Some(PathBuf::from("E:/configured")),
            &always_writable,
            &no_registry_anywhere,
        );
        assert_eq!(root.path, PathBuf::from("E:/configured"));
        assert_eq!(root.source, EngineRootSource::UserSetting);
    }

    #[test]
    fn defaults_next_to_the_app_when_writable() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Apps/Veil")),
            None,
            None,
            &always_writable,
            &no_registry_anywhere,
        );
        assert_eq!(root.path, PathBuf::from("C:/Apps/Veil/runtime"));
        assert_eq!(root.source, EngineRootSource::NextToApp);
    }

    /// The `C:\Program Files` case: the install directory exists and is not writable, so the
    /// engine must not be promised a home it cannot have.
    #[test]
    fn falls_back_to_data_dir_when_the_install_directory_is_read_only() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Program Files/Veil")),
            None,
            None,
            &never_writable,
            &no_registry_anywhere,
        );
        assert_eq!(root.path, PathBuf::from("D:/data/runtime"));
        assert_eq!(root.source, EngineRootSource::DataDir);
    }

    #[test]
    fn a_cargo_target_directory_is_never_used_as_the_install_directory() {
        for exe_dir in [
            "D:/src/Veil/target/debug",
            "D:/src/Veil/target/release",
            "D:/src/Veil/target/x86_64-pc-windows-msvc/debug",
        ] {
            let root = resolve_engine_root(
                Path::new("D:/data"),
                Some(Path::new(exe_dir)),
                None,
                None,
                &always_writable,
                &no_registry_anywhere,
            );
            assert_eq!(
                root.source,
                EngineRootSource::DevBuild,
                "{exe_dir} should not host the engine"
            );
        }
    }

    /// An installed app whose directory happens to be named `release` is not a Cargo build.
    #[test]
    fn only_a_target_parent_marks_a_build_directory() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Apps/release")),
            None,
            None,
            &always_writable,
            &no_registry_anywhere,
        );
        assert_eq!(root.source, EngineRootSource::NextToApp);
    }

    /// The upgrade case. An engine installed under the old `<data_dir>\runtime` layout must keep
    /// working after the default moved next to the app: reporting "no engine installed" because
    /// the new default directory is empty would be a regression, and moving 205 MB during startup
    /// would stall the splash screen. So it is adopted where it is, and Settings offers the move.
    #[test]
    fn adopts_the_historical_root_when_it_holds_the_only_engine() {
        let legacy = PathBuf::from("D:/data/runtime");
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Apps/Veil")),
            None,
            None,
            &always_writable,
            &|path| path == legacy,
        );
        assert_eq!(root.path, legacy);
        assert_eq!(root.source, EngineRootSource::LegacyAdopted);
    }

    /// Once something is installed next to the app, that is the root — even if the old location
    /// still has files, which is the state a half-finished manual copy leaves behind.
    #[test]
    fn prefers_next_to_the_app_once_an_engine_is_installed_there() {
        let root = resolve_engine_root(
            Path::new("D:/data"),
            Some(Path::new("C:/Apps/Veil")),
            None,
            None,
            &always_writable,
            &|_| true,
        );
        assert_eq!(root.path, PathBuf::from("C:/Apps/Veil/runtime"));
        assert_eq!(root.source, EngineRootSource::NextToApp);
    }
}
