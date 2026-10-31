use tauri::State;
use veil_core::{AppError, AppResult, ErrorCode};

use crate::app_state::AppState;
use crate::services::{
    CreateProxyInput, DiagnosticAction, DiagnosticComponent, Proxy, ProxyCredentials,
    ProxyTestResult, SensitiveActionInput, UpdateProxyInput,
};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealProxyCredentialsInput {
    pub proxy_id: String,
    pub verification: SensitiveActionInput,
}

#[tauri::command]
pub fn list_proxies(state: State<'_, AppState>) -> AppResult<Vec<Proxy>> {
    state.proxies()?.list()
}

#[tauri::command]
pub fn get_proxy(state: State<'_, AppState>, proxy_id: String) -> AppResult<Proxy> {
    state.proxies()?.get(&proxy_id)
}

#[tauri::command]
pub fn create_proxy(state: State<'_, AppState>, input: CreateProxyInput) -> AppResult<Proxy> {
    state.proxies()?.create(input)
}

#[tauri::command]
pub fn update_proxy(
    state: State<'_, AppState>,
    proxy_id: String,
    input: UpdateProxyInput,
) -> AppResult<Proxy> {
    state.proxies()?.update(&proxy_id, input)
}

#[tauri::command]
pub fn delete_proxy(state: State<'_, AppState>, proxy_id: String) -> AppResult<()> {
    state.proxies()?.delete(&proxy_id)
}

#[tauri::command]
pub async fn reveal_proxy_credentials(
    state: State<'_, AppState>,
    input: RevealProxyCredentialsInput,
) -> AppResult<ProxyCredentials> {
    let onboarding = state.onboarding.clone();
    let proxies = state.proxies()?;
    tauri::async_runtime::spawn_blocking(move || {
        onboarding.verify_sensitive_action(input.verification)?;
        let record = proxies.get_record(&input.proxy_id)?;
        Ok(ProxyCredentials::from(record))
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?
}

/// Apply a single proxy id (or `None` to clear) to a batch of profiles. Best-effort:
/// a single bad profile id never blocks the rest. Returns the number of profiles whose
/// assignment was changed successfully.
#[tauri::command]
pub fn batch_set_profile_proxy(
    state: State<'_, AppState>,
    profile_ids: Vec<String>,
    proxy_id: Option<String>,
) -> AppResult<usize> {
    let proxies = state.proxies()?;
    let target = proxy_id.as_deref();
    let mut changed = 0usize;
    for profile_id in &profile_ids {
        if proxies.set_profile_proxy(profile_id, target).is_ok() {
            changed += 1;
        }
    }
    Ok(changed)
}

/// Run the user-initiated connectivity probe through the proxy. Blocking HTTP runs on
/// `spawn_blocking` so it never stalls the async runtime. Failures resolve to
/// `ProxyTestResult { ok: false, code: Some("proxy.testFailed") }` rather than command
/// errors — the row should keep rendering, just with a sad badge.
#[tauri::command]
pub async fn test_proxy(
    state: State<'_, AppState>,
    proxy_id: String,
) -> AppResult<ProxyTestResult> {
    let diagnostics = state.diagnostics();
    // Resolve credentials on the calling task; only the small `ProxyConfig` crosses
    // into the worker thread.
    let result: AppResult<ProxyTestResult> = async {
        let proxies = state.proxies()?;
        let record = proxies.get_record(&proxy_id)?;
        let cfg = veil_runtime::ProxyConfig {
            kind: record.kind,
            host: record.host,
            port: record.port,
            username: record.username,
            password: record.password,
        };
        let outcome = tauri::async_runtime::spawn_blocking(move || veil_runtime::test_proxy(&cfg))
            .await
            .map_err(|_| veil_core::AppError::from(veil_core::ErrorCode::Unknown))?;
        Ok(match outcome {
            Ok(o) => {
                let _ = proxies.update_location(&proxy_id, &o);
                ProxyTestResult::success(o)
            }
            Err(_) => ProxyTestResult::failure("proxy.testFailed"),
        })
    }
    .await;
    match &result {
        Ok(test) if test.ok => {
            diagnostics.record_ok(DiagnosticComponent::Proxy, DiagnosticAction::Test)
        }
        Ok(test) => diagnostics.record_failure_code(
            DiagnosticComponent::Proxy,
            DiagnosticAction::Test,
            test.code.as_deref().unwrap_or("proxy.testFailed"),
        ),
        Err(err) => diagnostics.record_failure(
            DiagnosticComponent::Proxy,
            DiagnosticAction::Test,
            err.code(),
        ),
    }
    result
}
