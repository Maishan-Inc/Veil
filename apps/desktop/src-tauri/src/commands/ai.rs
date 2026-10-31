use tauri::State;
use veil_core::{AppError, AppResult, ErrorCode};

use crate::app_state::AppState;
use crate::services::{
    AiAuditLog, AiAuditLogFilter, AiAutomationSession, AiModelConfig, AiProfileFact, AiSettings,
    AiTask, AiTaskEvent, ApproveAiTaskInput, ExportAiAuditLogsResult, RunAiTaskResult,
    SensitiveActionInput, StartAiTaskInput, StartAiTaskResult, UpdateAiSettingsInput,
    UpsertAiModelConfigInput, UpsertAiProfileFactInput,
};

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>) -> AppResult<AiSettings> {
    state.ai()?.get_settings()
}

#[tauri::command]
pub async fn update_ai_settings(
    state: State<'_, AppState>,
    input: UpdateAiSettingsInput,
) -> AppResult<AiSettings> {
    let ai = state.ai()?;
    ai.update_settings(input)
}

#[tauri::command]
pub fn has_ai_provider_credentials(state: State<'_, AppState>) -> AppResult<bool> {
    state.ai()?.has_provider_credentials()
}

#[tauri::command]
pub fn list_ai_model_configs(state: State<'_, AppState>) -> AppResult<Vec<AiModelConfig>> {
    state.ai()?.list_model_configs()
}

#[tauri::command]
pub fn upsert_ai_model_config(
    state: State<'_, AppState>,
    input: UpsertAiModelConfigInput,
) -> AppResult<AiModelConfig> {
    state.ai()?.upsert_model_config(input)
}

#[tauri::command]
pub fn list_ai_audit_logs(
    state: State<'_, AppState>,
    filter: AiAuditLogFilter,
) -> AppResult<Vec<AiAuditLog>> {
    state.ai()?.list_audit_logs(filter)
}

#[tauri::command]
pub async fn export_ai_audit_logs(
    state: State<'_, AppState>,
    filter: AiAuditLogFilter,
    verification: SensitiveActionInput,
) -> AppResult<ExportAiAuditLogsResult> {
    let onboarding = state.onboarding.clone();
    let ai = state.ai()?;
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || onboarding.verify_sensitive_action(verification))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))??;
    ai.export_audit_logs(&data_dir, filter)
}

#[tauri::command]
pub fn list_ai_tasks(state: State<'_, AppState>, profile_id: String) -> AppResult<Vec<AiTask>> {
    state.ai()?.list_tasks_for_profile(&profile_id)
}

#[tauri::command]
pub fn get_ai_task(state: State<'_, AppState>, task_id: String) -> AppResult<AiTask> {
    state.ai()?.get_task(&task_id)
}

#[tauri::command]
pub fn list_ai_task_events(
    state: State<'_, AppState>,
    task_id: String,
) -> AppResult<Vec<AiTaskEvent>> {
    state.ai()?.list_task_events(&task_id)
}

#[tauri::command]
pub fn decide_ai_task_approval(
    state: State<'_, AppState>,
    input: ApproveAiTaskInput,
) -> AppResult<AiTask> {
    let ai = state.ai()?;
    let task = ai.decide_task_approval(input)?;
    if task.status == "ready" {
        ai.run_ready_task_if_needed(&task.id, None)?;
    }
    ai.get_task(&task.id)
}

#[tauri::command]
pub fn run_ai_task(state: State<'_, AppState>, task_id: String) -> AppResult<RunAiTaskResult> {
    state.ai()?.run_ready_task(&task_id, None)
}

#[tauri::command]
pub fn pause_ai_task_for_human_verification(
    state: State<'_, AppState>,
    task_id: String,
) -> AppResult<AiTask> {
    state.ai()?.pause_for_human_verification(&task_id)
}

#[tauri::command]
pub fn list_ai_profile_facts(
    state: State<'_, AppState>,
    profile_id: String,
) -> AppResult<Vec<AiProfileFact>> {
    state.ai()?.list_profile_facts(&profile_id)
}

#[tauri::command]
pub fn upsert_ai_profile_fact(
    state: State<'_, AppState>,
    input: UpsertAiProfileFactInput,
) -> AppResult<AiProfileFact> {
    state.ai()?.upsert_profile_fact(input)
}

#[tauri::command]
pub fn get_ai_session(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<AiAutomationSession> {
    state.ai()?.get_session(&session_id)
}

pub fn finalize_ai_task_start(
    state: &AppState,
    input: StartAiTaskInput,
    session: AiAutomationSession,
) -> AppResult<StartAiTaskResult> {
    let task = state.ai()?.start_task(input, &session)?;
    if task.status == "ready" {
        state.ai()?.run_ready_task_if_needed(&task.id, None)?;
    }
    Ok(StartAiTaskResult { task, session })
}
