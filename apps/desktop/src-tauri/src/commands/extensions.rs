use tauri::State;
use veil_core::AppResult;

use crate::app_state::AppState;
use crate::services::{
    BrowserExtension, DiagnosticAction, DiagnosticComponent, ImportExtensionFromLocalInput,
    ImportExtensionFromStoreInput, UpdateExtensionInput,
};

#[tauri::command]
pub fn list_extensions(state: State<'_, AppState>) -> AppResult<Vec<BrowserExtension>> {
    state.extensions()?.list()
}

#[tauri::command]
pub fn get_extension(
    state: State<'_, AppState>,
    extension_id: String,
) -> AppResult<BrowserExtension> {
    state.extensions()?.get(&extension_id)
}

#[tauri::command]
pub async fn import_extension_from_store(
    state: State<'_, AppState>,
    input: ImportExtensionFromStoreInput,
) -> AppResult<BrowserExtension> {
    let diagnostics = state.diagnostics();
    let service = state.extensions()?;
    let result = match tauri::async_runtime::spawn_blocking(move || {
        service.import_from_store(input)
    })
    .await
    {
        Ok(inner) => inner,
        Err(_) => Err(veil_core::AppError::from(
            veil_core::ErrorCode::ExtensionDownloadFailed,
        )),
    };
    match &result {
        Ok(_) => diagnostics.record_ok(DiagnosticComponent::Extensions, DiagnosticAction::Import),
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Extensions,
            DiagnosticAction::Import,
            err.code(),
        ),
    }
    result
}

#[tauri::command]
pub async fn import_extension_from_local(
    state: State<'_, AppState>,
    input: ImportExtensionFromLocalInput,
) -> AppResult<BrowserExtension> {
    let diagnostics = state.diagnostics();
    let service = state.extensions()?;
    let result = match tauri::async_runtime::spawn_blocking(move || {
        service.import_from_local(input)
    })
    .await
    {
        Ok(inner) => inner,
        Err(_) => Err(veil_core::AppError::from(
            veil_core::ErrorCode::ExtensionInvalid,
        )),
    };
    match &result {
        Ok(_) => diagnostics.record_ok(DiagnosticComponent::Extensions, DiagnosticAction::Import),
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Extensions,
            DiagnosticAction::Import,
            err.code(),
        ),
    }
    result
}

#[tauri::command]
pub fn update_extension(
    state: State<'_, AppState>,
    extension_id: String,
    input: UpdateExtensionInput,
) -> AppResult<BrowserExtension> {
    state.extensions()?.update(&extension_id, input)
}

#[tauri::command]
pub fn delete_extension(state: State<'_, AppState>, extension_id: String) -> AppResult<()> {
    state.extensions()?.delete(&extension_id)
}
