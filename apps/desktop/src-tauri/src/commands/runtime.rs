use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use veil_core::{AppResult, ErrorCode};
use veil_runtime::FlagDialect;

use crate::app_state::AppState;
use crate::engine_root::EngineRootSource;
use crate::services::{
    AvailableRuntimePayload, DiagnosticAction, DiagnosticComponent, RuntimeProgress, RuntimeState,
    RuntimeVersionPayload,
};

/// Installed engines plus the capability facts the fingerprint surface needs.
///
/// `RuntimeState` is a tagged `Missing | Installed` enum, which makes a field shared by both
/// variants awkward; this payload is already fetched by every screen that needs the engine, so
/// the dialect rides along here instead of behind a new command.
///
/// All three capability fields describe the **currently selected installed engine**, not
/// `engine.toml`. Reporting the manifest's dialect here is what let an engine installed before the
/// 2026-08-23 flip be presented as fully patched.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntimes {
    pub current_version: Option<String>,
    pub installed: Vec<RuntimeVersionPayload>,
    /// Name of the selected engine as recorded at install, or the manifest's name when there is no
    /// selection. Named in the degraded-capability banner, so it must describe the same engine as
    /// `flag_dialect`.
    pub engine_name: String,
    /// Switch vocabulary the selected engine implements. `stock-chromium` means the fingerprint
    /// surface must show a degraded-capability state.
    pub flag_dialect: FlagDialect,
    /// Whether `flag_dialect` was read off that engine (`true`) or defaulted because nothing is
    /// recorded (`false`). The UI needs the distinction: both behave as `stock-chromium`, but only
    /// the first justifies telling the user their engine cannot spoof.
    pub dialect_recorded: bool,
}

#[tauri::command]
pub fn get_runtime_state(state: State<'_, AppState>) -> RuntimeState {
    state.runtime.state()
}

/// Where engine versions are installed, and which rule put them there.
///
/// The reason travels with the path because the path alone cannot be argued with: a user who
/// asked for the engine to live beside the app needs to see when Veil could not honour that —
/// an unwritable `C:\Program Files` falls back silently otherwise.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRootInfo {
    pub path: String,
    pub source: &'static str,
}

#[tauri::command]
pub fn get_runtime_install_root(state: State<'_, AppState>) -> EngineRootInfo {
    EngineRootInfo::new(&state)
}

/// Move the installed engines to `path` and remember it as the user's choice.
///
/// Runs on a blocking thread: a cross-volume move copies ~200 MB per installed version, and
/// holding the IPC thread for that would freeze the window.
#[tauri::command]
pub async fn set_runtime_install_root(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<EngineRootInfo> {
    let target = std::path::PathBuf::from(path.trim());
    let moved = state.move_engine_root(target)?;
    Ok(EngineRootInfo {
        path: state.runtime.install_root_path(),
        source: source_label(moved.source),
    })
}

impl EngineRootInfo {
    fn new(state: &AppState) -> Self {
        Self {
            path: state.runtime.install_root_path(),
            source: source_label(state.engine_root().source),
        }
    }
}

fn source_label(source: EngineRootSource) -> &'static str {
    match source {
        EngineRootSource::EnvOverride => "envOverride",
        EngineRootSource::UserSetting => "userSetting",
        EngineRootSource::NextToApp => "nextToApp",
        EngineRootSource::LegacyAdopted => "legacyAdopted",
        EngineRootSource::DevBuild => "devBuild",
        EngineRootSource::DataDir => "dataDir",
    }
}

#[tauri::command]
pub fn list_installed_runtimes(state: State<'_, AppState>) -> AppResult<InstalledRuntimes> {
    let snapshot = state.runtime.installed_snapshot()?;
    Ok(InstalledRuntimes {
        current_version: snapshot.current_version,
        installed: snapshot.installed,
        engine_name: snapshot.capability.engine_name,
        flag_dialect: snapshot.capability.flag_dialect,
        dialect_recorded: snapshot.capability.dialect_recorded,
    })
}

/// Fetch the available engine versions from GitHub Releases. Network call — gated by the
/// manifest: a placeholder `engine.toml` short-circuits to `runtime.assetNotFound`.
#[tauri::command]
pub async fn list_available_runtimes(
    state: State<'_, AppState>,
) -> AppResult<Vec<AvailableRuntimePayload>> {
    let runtime = state.runtime.clone();
    tauri::async_runtime::spawn_blocking(move || runtime.list_available())
        .await
        .unwrap_or_else(|_| Err(veil_core::ErrorCode::Unknown.into()))
}

/// Install a runtime version. The candidate must come from a prior `list_available_runtimes`
/// call. Progress is streamed via the Channel; the call resolves only when the pipeline
/// finishes (success / fail / cancel).
///
/// The pipeline is **blocking** (`reqwest::blocking` inside `installer::download_streaming`), so
/// it must run on the blocking pool, exactly as `list_available_runtimes` does. Calling it
/// directly from this `async fn` put a blocking client on a tokio worker: dropping it there
/// panics with *"Cannot drop a runtime in a context where blocking is not allowed"*, the command
/// future dies before the first chunk, and the UI sits on `Downloading { downloaded: 0 }`
/// forever because no further event is ever sent and the promise never settles.
#[tauri::command]
pub async fn install_runtime_version(
    state: State<'_, AppState>,
    candidate: AvailableRuntimePayload,
    on_progress: Channel<RuntimeProgress>,
) -> AppResult<RuntimeVersionPayload> {
    let runtime = state.runtime.clone();
    let diagnostics = state.diagnostics();
    let result = tauri::async_runtime::spawn_blocking(move || {
        runtime.install(&candidate, move |event| {
            // Best-effort send — failure means the channel was dropped (window closed, etc.).
            let _ = on_progress.send(event);
        })
    })
    .await
    .unwrap_or_else(|_| Err(veil_core::ErrorCode::Unknown.into()));
    match &result {
        Ok(_) => diagnostics.record_ok(DiagnosticComponent::Engine, DiagnosticAction::Install),
        Err(err) if err.code() == ErrorCode::RuntimeInstallCancelled => {
            diagnostics.record_cancelled(DiagnosticComponent::Engine, DiagnosticAction::Install)
        }
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Engine,
            DiagnosticAction::Install,
            err.code(),
        ),
    }
    result
}

/// Install a runtime from a local ZIP path supplied by the user. The version/checksum metadata
/// must still come from `list_available_runtimes`; the local file is verified before extraction.
///
/// Blocking for the same reason as `install_runtime_version`: no download, but the SHA-256 read
/// and the zip extraction are both synchronous file I/O over ~200 MB.
#[tauri::command]
pub async fn install_runtime_from_local_zip(
    state: State<'_, AppState>,
    candidate: AvailableRuntimePayload,
    local_zip_path: String,
    on_progress: Channel<RuntimeProgress>,
) -> AppResult<RuntimeVersionPayload> {
    let runtime = state.runtime.clone();
    let diagnostics = state.diagnostics();
    let result = tauri::async_runtime::spawn_blocking(move || {
        runtime.install_from_local_zip(&candidate, &local_zip_path, move |event| {
            let _ = on_progress.send(event);
        })
    })
    .await
    .unwrap_or_else(|_| Err(veil_core::ErrorCode::Unknown.into()));
    match &result {
        Ok(_) => diagnostics.record_ok(DiagnosticComponent::Engine, DiagnosticAction::Install),
        Err(err) if err.code() == ErrorCode::RuntimeInstallCancelled => {
            diagnostics.record_cancelled(DiagnosticComponent::Engine, DiagnosticAction::Install)
        }
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Engine,
            DiagnosticAction::Install,
            err.code(),
        ),
    }
    result
}

#[tauri::command]
pub fn cancel_runtime_install(state: State<'_, AppState>, version: String) -> AppResult<()> {
    state.runtime.cancel(&version);
    Ok(())
}

#[tauri::command]
pub fn set_current_runtime(state: State<'_, AppState>, version: String) -> AppResult<RuntimeState> {
    state.runtime.set_current(&version)
}

#[tauri::command]
pub fn uninstall_runtime_version(state: State<'_, AppState>, version: String) -> AppResult<()> {
    let result = state.runtime.uninstall(&version);
    match &result {
        Ok(()) => state
            .diagnostics()
            .record_ok(DiagnosticComponent::Engine, DiagnosticAction::Uninstall),
        Err(err) => state.diagnostics().record_failure(
            DiagnosticComponent::Engine,
            DiagnosticAction::Uninstall,
            err.code(),
        ),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the wire name of the capability fields.
    ///
    /// `@veil/shared`'s `InstalledRuntimes` reads `dialectRecorded`, and a missing key deserializes to
    /// `undefined` — which is falsy, so a snake_case slip here would put the *unrecorded* banner in
    /// front of every user including the ones on a properly recorded patched engine. Nothing else in
    /// the suite crosses that boundary: the Rust tests stop at `EngineCapability` and the React test
    /// starts at the prop.
    #[test]
    fn the_capability_fields_serialize_in_the_camel_case_the_ui_reads() {
        let json = serde_json::to_value(InstalledRuntimes {
            current_version: Some("151.0.7922.137-1.1001".to_string()),
            installed: Vec::new(),
            engine_name: "Veil-Chromium".to_string(),
            flag_dialect: FlagDialect::VeilPatched,
            dialect_recorded: true,
        })
        .expect("serialize");

        assert_eq!(json["dialectRecorded"], serde_json::json!(true));
        assert_eq!(json["flagDialect"], serde_json::json!("veil-patched"));
        assert_eq!(json["engineName"], serde_json::json!("Veil-Chromium"));
        assert_eq!(
            json["currentVersion"],
            serde_json::json!("151.0.7922.137-1.1001")
        );
        assert!(
            json.get("dialect_recorded").is_none(),
            "snake_case must not leak: {json}"
        );
    }
}
