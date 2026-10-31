use std::path::PathBuf;

use tauri::State;
use veil_core::{AppError, AppResult, ErrorCode};

use crate::app_state::AppState;
use crate::services::{DiagnosticsBundle, EngineInfo, ExportDiagnosticsResult};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDiagnosticsInput {
    /// User-chosen output path (from the frontend's save dialog).
    pub path: String,
    pub include_graphics_details: bool,
}

/// Resolve the installed engine's version and flag dialect for the bundle manifest. Best-effort:
/// a fresh install with nothing recorded yields `None` fields, which is itself the fact the
/// manifest carries.
fn engine_info(state: &AppState) -> EngineInfo {
    match state.runtime.installed_snapshot() {
        Ok(snapshot) => EngineInfo {
            version: snapshot.current_version,
            flag_dialect: serde_json::to_value(snapshot.capability.flag_dialect)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string)),
        },
        Err(_) => EngineInfo::default(),
    }
}

/// Return the bundle the user will preview before exporting. Export writes this exact content.
#[tauri::command]
pub async fn preview_diagnostics(state: State<'_, AppState>) -> AppResult<DiagnosticsBundle> {
    let service = state.diagnostics();
    let engine = engine_info(&state);
    tauri::async_runtime::spawn_blocking(move || service.preview(engine))
        .await
        .map_err(|_| AppError::from(ErrorCode::DiagnosticsUnavailable))?
}

/// Write the diagnostic bundle to a user-chosen path. Local only — no network, no retained copy.
#[tauri::command]
pub async fn export_diagnostics(
    state: State<'_, AppState>,
    input: ExportDiagnosticsInput,
) -> AppResult<ExportDiagnosticsResult> {
    let path = PathBuf::from(input.path.trim());
    if path.as_os_str().is_empty() {
        return Err(AppError::from(ErrorCode::DiagnosticsExportFailed));
    }
    let service = state.diagnostics();
    let engine = engine_info(&state);
    let include_graphics = input.include_graphics_details;
    tauri::async_runtime::spawn_blocking(move || service.export(&path, engine, include_graphics))
        .await
        .map_err(|_| AppError::from(ErrorCode::DiagnosticsExportFailed))?
}
