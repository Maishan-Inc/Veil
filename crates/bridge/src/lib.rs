//! Engine bridge: the only place that turns a profile into a launched browser process.
//!
//! The bridge maps Veil profile data to engine launch arguments and spawns/stops the
//! process. It deliberately contains **no** fingerprint or browser-core logic — that is
//! the engine's job. The concrete engine is provided by `veil-runtime` and is pluggable.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("engine executable is missing")]
    ExecutableMissing,
    /// The engine cannot start *from where it is installed*.
    ///
    /// Chromium's `chrome.exe` declares a private side-by-side assembly named after its version and
    /// resolved from a `<version>.manifest` beside the executable. That resolution can fail for a
    /// whole directory subtree while a byte-identical copy elsewhere starts fine — measured on the
    /// development host, where every `chrome.exe` under the user profile fails and the same three
    /// files under `C:\` succeed.
    ///
    /// Separated from [`BridgeError::Spawn`] because the two need opposite advice: a generic spawn
    /// failure suggests re-installing, and re-installing into the same root cannot fix this one.
    #[error("engine cannot start from its install location (side-by-side activation failed)")]
    ActivationContextFailed,
    #[error("failed to spawn engine process")]
    Spawn(#[from] std::io::Error),
}

/// `ERROR_SXS_CANT_GEN_ACTCTX` — "the application has failed to start because its side-by-side
/// configuration is incorrect".
#[cfg(windows)]
const ERROR_SXS_CANT_GEN_ACTCTX: i32 = 14001;

/// Everything the bridge needs to launch one profile.
#[derive(Debug, Clone)]
pub struct LaunchRequest {
    /// Path to the engine executable resolved from the runtime registry.
    pub executable_path: PathBuf,
    /// Per-profile, isolated Chromium user-data directory.
    pub user_data_dir: PathBuf,
    pub profile_id: String,
    /// Engine-specific extra arguments (e.g. proxy/fingerprint flags) built by services.
    pub extra_args: Vec<String>,
    /// Environment variables to set on the child process, for values the engine reads from the
    /// environment rather than from a switch (currently `TZ`). Kept separate from `extra_args`
    /// on purpose: env must not be smuggled through argv.
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchResult {
    pub process_id: u32,
}

/// A running engine process handle.
#[derive(Debug)]
pub struct EngineProcess {
    child: Child,
}

impl EngineProcess {
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Best-effort terminate.
    pub fn stop(&mut self) -> Result<(), BridgeError> {
        self.child.kill().map_err(BridgeError::Spawn)
    }

    /// Non-blocking liveness check. Returns `true` if the process is still running,
    /// `false` if it has exited (and reaps its exit status). The bridge does not surface
    /// the exit code — the caller only needs to know whether the handle is still useful.
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            // Exited or impossible to wait — treat the handle as dead either way so the
            // caller can drop it from any tracking map.
            Ok(Some(_)) | Err(_) => false,
        }
    }
}

/// Build the engine command-line arguments for a launch request.
///
/// Each profile gets its own `--user-data-dir` for full isolation, plus flags that keep
/// automated launches quiet. Profile-specific flags are appended verbatim.
pub fn build_engine_args(request: &LaunchRequest) -> Vec<String> {
    let mut args = vec![
        format!(
            "--user-data-dir={}",
            normalize_arg_path(&request.user_data_dir)
        ),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ];
    args.extend(request.extra_args.iter().cloned());
    args
}

/// Assemble the child process command: engine executable, launch arguments, and the request's
/// environment overlay on top of the inherited environment.
fn build_command(request: &LaunchRequest) -> Command {
    let mut command = Command::new(&request.executable_path);
    command.args(build_engine_args(request));
    command.envs(
        request
            .env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    command
}

/// Launch the engine for a profile and return its process id.
pub fn launch(request: &LaunchRequest) -> Result<(EngineProcess, LaunchResult), BridgeError> {
    if !request.executable_path.exists() {
        return Err(BridgeError::ExecutableMissing);
    }
    let child = build_command(request)
        .spawn()
        .map_err(classify_spawn_error)?;
    let result = LaunchResult {
        process_id: child.id(),
    };
    Ok((EngineProcess { child }, result))
}

/// Keep the OS-specific reading of a spawn failure here rather than in a service: this crate is the
/// only one that calls `CreateProcess`, and the raw error code is lost the moment it is mapped.
fn classify_spawn_error(err: std::io::Error) -> BridgeError {
    #[cfg(windows)]
    if err.raw_os_error() == Some(ERROR_SXS_CANT_GEN_ACTCTX) {
        return BridgeError::ActivationContextFailed;
    }
    BridgeError::Spawn(err)
}

fn normalize_arg_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LaunchRequest {
        LaunchRequest {
            executable_path: PathBuf::from("C:/Veil/runtime/120/engine.exe"),
            user_data_dir: PathBuf::from("C:\\Veil\\profiles\\profile-1"),
            profile_id: "profile-1".to_string(),
            extra_args: vec!["--proxy-server=socks5://127.0.0.1:1080".to_string()],
            env: Vec::new(),
        }
    }

    #[test]
    fn maps_profile_to_isolated_user_data_dir() {
        let args = build_engine_args(&sample());
        assert_eq!(args[0], "--user-data-dir=C:/Veil/profiles/profile-1");
        assert!(args.iter().any(|a| a == "--no-first-run"));
        assert!(args
            .iter()
            .any(|a| a == "--proxy-server=socks5://127.0.0.1:1080"));
    }

    #[test]
    fn appends_fingerprint_seed_to_launch_args() {
        let mut request = sample();
        request.extra_args.push("--fingerprint=424242".to_string());

        let args = build_engine_args(&request);

        assert!(args.iter().any(|a| a == "--fingerprint=424242"));
    }

    #[test]
    fn env_never_leaks_into_the_argument_list() {
        // `TZ` reaches Chromium through the environment; smuggling it through argv would emit a
        // switch that does not exist.
        let mut request = sample();
        request
            .env
            .push(("TZ".to_string(), "Asia/Tokyo".to_string()));

        let args = build_engine_args(&request);

        assert_eq!(args, build_engine_args(&sample()));
        assert!(!args.iter().any(|a| a.contains("TZ")));
        assert!(!args.iter().any(|a| a.contains("Asia/Tokyo")));
    }

    #[test]
    fn launch_applies_env_to_the_child_process() {
        let mut request = sample();
        request
            .env
            .push(("TZ".to_string(), "Asia/Tokyo".to_string()));

        // `launch` spawns exactly this command; inspecting it avoids starting a real browser.
        let command = build_command(&request);
        let env: Vec<(String, Option<String>)> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();

        assert_eq!(
            env,
            vec![("TZ".to_string(), Some("Asia/Tokyo".to_string()))]
        );
    }

    #[test]
    fn launch_without_env_leaves_the_inherited_environment_alone() {
        let command = build_command(&sample());
        assert_eq!(command.get_envs().count(), 0);
    }

    #[test]
    fn launch_fails_when_executable_is_missing() {
        let err = launch(&sample()).unwrap_err();
        assert!(matches!(err, BridgeError::ExecutableMissing));
    }

    /// The classifier is what decides whether the UI tells the user to re-install (useless here) or
    /// to move the engine out of its current directory (the only thing that works). Asserted on the
    /// raw code rather than by triggering a real side-by-side failure, which needs a broken host.
    #[cfg(windows)]
    #[test]
    fn a_side_by_side_failure_is_not_reported_as_a_generic_spawn_failure() {
        let err =
            classify_spawn_error(std::io::Error::from_raw_os_error(ERROR_SXS_CANT_GEN_ACTCTX));
        assert!(
            matches!(err, BridgeError::ActivationContextFailed),
            "got {err:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn other_spawn_failures_stay_generic() {
        // ERROR_ACCESS_DENIED — a real spawn failure with different advice.
        let err = classify_spawn_error(std::io::Error::from_raw_os_error(5));
        assert!(matches!(err, BridgeError::Spawn(_)), "got {err:?}");
    }
}
