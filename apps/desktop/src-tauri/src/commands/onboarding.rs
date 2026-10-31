use tauri::State;
use veil_core::{AppError, AppResult, ErrorCode};
use veil_secure_storage::VaultKeystore;

use crate::app_state::{
    map_vault_key_error, open_database_service_pair, AppState, AppStateInitError, VaultStatus,
};
use crate::local_api::LocalApiServer;
use crate::services::{
    CompleteOnboardingInput, DiagnosticAction, DiagnosticComponent, EncryptionMode,
    OnboardingService, OnboardingStatus, RecoverSecurityInput, RecoveryPhraseOutput,
    SensitiveActionInput, TotpEnrollmentOutput,
};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockVaultInput {
    pub password: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginTotpEnrollmentInput {
    pub verification: SensitiveActionInput,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmTotpEnrollmentInput {
    pub secret: String,
    pub code: String,
    pub verification: SensitiveActionInput,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverVaultInput {
    pub recovery_phrase: Vec<String>,
    pub security: RecoverSecurityInput,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRecoveryPhraseTxtInput {
    pub path: String,
    pub contents: String,
}

#[tauri::command]
pub fn get_onboarding_status(state: State<'_, AppState>) -> OnboardingStatus {
    state.onboarding.status()
}

#[tauri::command]
pub async fn complete_onboarding(
    state: State<'_, AppState>,
    input: CompleteOnboardingInput,
) -> AppResult<OnboardingStatus> {
    let service = state.onboarding.clone();
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        OnboardingService::validate_complete_input(&input)?;
        if input.encryption_mode == EncryptionMode::Standard {
            let password = input
                .password
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::from(ErrorCode::StorageEncryptionFailed))?;
            VaultKeystore::new(&data_dir)
                .protect_with_password_from_dpapi(password.as_bytes())
                .map_err(|err| AppError::from(map_vault_key_error(err)))?;
        }
        if !input.mnemonic_enabled {
            VaultKeystore::new(&data_dir)
                .clear_recovery_phrase()
                .map_err(|err| AppError::from(map_vault_key_error(err)))?;
        }
        service.complete(input)
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::StorageEncryptionFailed))?
}

#[tauri::command]
pub fn reset_onboarding(state: State<'_, AppState>) -> AppResult<OnboardingStatus> {
    state.onboarding.reset()
}

#[tauri::command]
pub async fn save_recovery_phrase_txt(input: SaveRecoveryPhraseTxtInput) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::PathBuf::from(input.path);
        if path.as_os_str().is_empty() {
            return Err(AppError::from(ErrorCode::StorageDatabaseWriteFailed));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        }
        std::fs::write(path, input.contents)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?
}

#[tauri::command]
pub async fn generate_recovery_phrase(
    state: State<'_, AppState>,
) -> AppResult<RecoveryPhraseOutput> {
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let phrase = veil_secure_storage::generate_recovery_phrase();
        VaultKeystore::new(&data_dir)
            .protect_with_recovery_phrase_from_dpapi(&phrase)
            .map_err(|err| AppError::from(map_vault_key_error(err)))?;
        Ok(RecoveryPhraseOutput { phrase })
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::StorageEncryptionFailed))?
}

#[tauri::command]
pub async fn verify_sensitive_action(
    state: State<'_, AppState>,
    input: SensitiveActionInput,
) -> AppResult<()> {
    let service = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || service.verify_sensitive_action(input))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?
}

#[tauri::command]
pub async fn begin_totp_enrollment(
    state: State<'_, AppState>,
    input: BeginTotpEnrollmentInput,
) -> AppResult<TotpEnrollmentOutput> {
    let service = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || service.begin_totp_enrollment(input.verification))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?
}

#[tauri::command]
pub async fn confirm_totp_enrollment(
    state: State<'_, AppState>,
    input: ConfirmTotpEnrollmentInput,
) -> AppResult<OnboardingStatus> {
    let service = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.confirm_totp_enrollment(&input.secret, &input.code, input.verification)
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?
}

#[tauri::command]
pub async fn disable_totp(
    state: State<'_, AppState>,
    input: SensitiveActionInput,
) -> AppResult<OnboardingStatus> {
    let service = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || service.disable_totp(input))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?
}

#[tauri::command]
pub fn get_vault_status(state: State<'_, AppState>) -> VaultStatus {
    state.vault_status()
}

#[tauri::command]
pub async fn unlock_vault(
    state: State<'_, AppState>,
    server: State<'_, LocalApiServer>,
    input: UnlockVaultInput,
) -> AppResult<VaultStatus> {
    let diagnostics = state.diagnostics();
    let data_dir = state.data_dir.clone();
    let password = input.password;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let key = VaultKeystore::new(&data_dir)
            .unlock_with_password(password.as_bytes())
            .map_err(|err| AppError::from(map_vault_key_error(err)))?;
        open_database_service_pair(&data_dir, &key).map_err(map_app_state_error)
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::StorageDecryptionFailed))
    .and_then(std::convert::identity);
    match &result {
        Ok(_) => diagnostics.record_ok(DiagnosticComponent::Vault, DiagnosticAction::Unlock),
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Vault,
            DiagnosticAction::Unlock,
            err.code(),
        ),
    }
    let services = result?;
    state.set_unlocked_database(services);
    // The database is now readable, so an enabled local API can start listening.
    server.reconcile(&state);
    Ok(state.vault_status())
}

#[tauri::command]
pub async fn recover_vault_with_recovery_phrase(
    state: State<'_, AppState>,
    server: State<'_, LocalApiServer>,
    input: RecoverVaultInput,
) -> AppResult<VaultStatus> {
    let diagnostics = state.diagnostics();
    let data_dir = state.data_dir.clone();
    let service = state.onboarding.clone();
    let security = input.security;
    let phrase = input.recovery_phrase;
    let result = tauri::async_runtime::spawn_blocking(move || {
        OnboardingService::validate_recover_security_input(&security)?;
        let password = security
            .password
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::as_bytes);
        let key = VaultKeystore::new(&data_dir)
            .recover_with_recovery_phrase(&phrase, password)
            .map_err(|err| AppError::from(map_vault_key_error(err)))?;
        service.recover_security(security)?;
        open_database_service_pair(&data_dir, &key).map_err(map_app_state_error)
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::StorageDecryptionFailed))
    .and_then(std::convert::identity);
    match &result {
        Ok(_) => diagnostics.record_ok(DiagnosticComponent::Vault, DiagnosticAction::Recover),
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Vault,
            DiagnosticAction::Recover,
            err.code(),
        ),
    }
    let services = result?;
    state.set_unlocked_database(services);
    server.reconcile(&state);
    Ok(state.vault_status())
}

fn map_app_state_error(err: AppStateInitError) -> AppError {
    match err {
        AppStateInitError::DataDir => ErrorCode::StorageDatabaseReadFailed.into(),
        AppStateInitError::Key(code) | AppStateInitError::Database(code) => code.into(),
    }
}
