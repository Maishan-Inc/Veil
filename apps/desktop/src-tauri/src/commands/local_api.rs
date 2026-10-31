use tauri::State;
use veil_core::{AppError, AppResult, ErrorCode};

use crate::app_state::AppState;
use crate::local_api::LocalApiServer;
use crate::services::{
    requires_external_access_verification, LocalApiKey, LocalApiKeyIssue, LocalApiSettings,
    SensitiveActionInput, UpdateLocalApiSettingsInput,
};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateLocalApiKeyInput {
    pub name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotateLocalApiKeyInput {
    pub key_id: String,
}

#[tauri::command]
pub fn get_local_api_settings(state: State<'_, AppState>) -> AppResult<LocalApiSettings> {
    state.local_api()?.settings()
}

#[tauri::command]
pub async fn update_local_api_settings(
    state: State<'_, AppState>,
    server: State<'_, LocalApiServer>,
    input: UpdateLocalApiSettingsInput,
    verification: Option<SensitiveActionInput>,
) -> AppResult<LocalApiSettings> {
    let service = state.local_api()?;
    let previous = service.settings()?;
    if requires_external_access_verification(&previous, &input) {
        let verification =
            verification.ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let onboarding = state.onboarding.clone();
        tauri::async_runtime::spawn_blocking(move || {
            onboarding.verify_sensitive_action(verification)
        })
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))??;
    }
    let saved = service.update_settings(input)?;
    // Persisted settings are now the source of truth: bring the listener in line with
    // them. A bind failure (e.g. the port is taken) surfaces to the user rather than
    // leaving the console claiming "enabled" while nothing listens.
    server.apply_settings(&saved)?;
    Ok(saved)
}

#[tauri::command]
pub fn list_local_api_keys(state: State<'_, AppState>) -> AppResult<Vec<LocalApiKey>> {
    state.local_api()?.list_keys()
}

#[tauri::command]
pub async fn generate_local_api_key(
    state: State<'_, AppState>,
    input: GenerateLocalApiKeyInput,
    verification: SensitiveActionInput,
) -> AppResult<LocalApiKeyIssue> {
    let onboarding = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || onboarding.verify_sensitive_action(verification))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))??;
    state.local_api()?.generate_key(input.name)
}

#[tauri::command]
pub async fn rotate_local_api_key(
    state: State<'_, AppState>,
    input: RotateLocalApiKeyInput,
    verification: SensitiveActionInput,
) -> AppResult<LocalApiKeyIssue> {
    let onboarding = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || onboarding.verify_sensitive_action(verification))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))??;
    state.local_api()?.rotate_key(input.key_id)
}
