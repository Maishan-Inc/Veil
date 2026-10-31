//! Endpoint handlers for the local API. Each handler maps one HTTP route onto the same
//! application services the Tauri commands use — the HTTP layer is a second surface over
//! identical business logic, never a reimplementation.
//!
//! Handlers return the `data` payload; the router wraps it in the `{ success, data }`
//! envelope and maps `AppError` to an HTTP status + `{ success, error }` body.

use serde::Deserialize;
use veil_core::{AppError, AppResult, ErrorCode, Fingerprint};

use crate::app_state::AppState;
use crate::commands::{LaunchCustomTemporaryProfileInput, LaunchTemporaryProfileInput};
use crate::services::{
    CreateGroupInput, CreateProfileInput, ImportExtensionFromLocalInput,
    ImportExtensionFromStoreInput, LaunchStatus, SensitiveActionInput, UpdateGroupInput,
    UpdateProfileCookiesInput, UpsertProfileCredentialInput,
};

use super::http::Request;

pub fn health(_state: &AppState) -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({
        "status": "ok",
        "service": "veil-local-api",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub fn list_profiles(state: &AppState, request: &Request) -> AppResult<serde_json::Value> {
    let profiles = state.profiles()?.list()?;
    let group_id = request.query_param("groupId");
    let status = request.query_param("status");
    let keyword = request.query_param("keyword");
    let filtered: Vec<_> = profiles
        .into_iter()
        .filter(|profile| group_id.map_or(true, |id| profile.group_id.as_deref() == Some(id)))
        .filter(|profile| status.map_or(true, |status| profile.status == status))
        .filter(|profile| {
            keyword.map_or(true, |needle| {
                needle.is_empty()
                    || profile.name.to_lowercase().contains(&needle.to_lowercase())
                    || profile
                        .platform
                        .to_lowercase()
                        .contains(&needle.to_lowercase())
                    || profile
                        .notes
                        .to_lowercase()
                        .contains(&needle.to_lowercase())
            })
        })
        .collect();
    to_json(&filtered)
}

pub fn get_profile(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    let profile = state.profiles()?.get(id)?;
    to_json(&profile)
}

pub fn create_profile(state: &AppState, request: &Request) -> AppResult<serde_json::Value> {
    let input: CreateProfileInput = parse_body(request)?;
    let profile = state.profiles()?.create(input)?;
    // Mirrors the `create_profile` command: freshly-created profiles inherit default
    // extensions, so a launch over the API starts from the same state as one from the UI.
    state
        .extensions()?
        .assign_defaults_to_profile(&profile.id)?;
    let profile = state.profiles()?.get(&profile.id)?;
    to_json(&profile)
}

pub fn update_profile(
    state: &AppState,
    id: &str,
    request: &Request,
) -> AppResult<serde_json::Value> {
    let input: CreateProfileInput = parse_body(request)?;
    let profile = state.profiles()?.update(id, input)?;
    to_json(&profile)
}

pub fn delete_profile(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    state.profiles()?.delete(id)?;
    Ok(serde_json::json!({ "profileId": id, "deleted": true }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintPatch {
    fingerprint: Fingerprint,
}

pub fn update_fingerprint(
    state: &AppState,
    id: &str,
    request: &Request,
) -> AppResult<serde_json::Value> {
    let patch: FingerprintPatch = parse_body(request)?;
    let profile = state
        .profiles()?
        .update_fingerprint(id, patch.fingerprint)?;
    to_json(&profile)
}

/// Sweep exited engine processes out of the tracked map, resetting their rows to `idle`,
/// so the running-state endpoints report the truth rather than a stale handle.
fn reap_dead(state: &AppState) {
    let dead: Vec<String> = {
        let mut launched = state.launched.lock().expect("launched lock poisoned");
        let mut dead = Vec::new();
        launched.retain(|id, process| {
            if process.is_alive() {
                true
            } else {
                dead.push(id.clone());
                false
            }
        });
        dead
    };
    if let Ok(profiles) = state.profiles() {
        for id in dead {
            let _ = profiles.set_status(&id, "idle", false);
        }
    }
}

pub fn running_profiles(state: &AppState) -> AppResult<serde_json::Value> {
    reap_dead(state);
    let launched = state.launched.lock().expect("launched lock poisoned");
    let running: Vec<serde_json::Value> = launched
        .iter()
        .map(|(profile_id, process)| {
            serde_json::json!({ "profileId": profile_id, "processId": process.id() })
        })
        .collect();
    Ok(serde_json::Value::Array(running))
}

pub fn is_profile_running(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    let running = state
        .launched
        .lock()
        .expect("launched lock poisoned")
        .contains_key(id);
    Ok(serde_json::json!({ "profileId": id, "running": running }))
}

pub fn close_profile(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    state.profiles()?.get(id)?;
    let profile = state.stop_profile_shared(id.to_string())?;
    Ok(serde_json::json!({ "profileId": id, "closed": true, "status": profile.status }))
}

pub fn list_groups(state: &AppState) -> AppResult<serde_json::Value> {
    to_json(&state.profiles()?.list_groups()?)
}

pub fn create_group(state: &AppState, request: &Request) -> AppResult<serde_json::Value> {
    let input: CreateGroupInput = parse_body(request)?;
    let group = state.profiles()?.create_group(input)?;
    to_json(&group)
}

pub fn update_group(state: &AppState, id: &str, request: &Request) -> AppResult<serde_json::Value> {
    let input: UpdateGroupInput = parse_body(request)?;
    let group = state.profiles()?.update_group(id, input)?;
    to_json(&group)
}

pub fn delete_group(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    state.profiles()?.delete_group(id)?;
    Ok(serde_json::json!({ "groupId": id, "deleted": true }))
}

pub fn list_extensions(state: &AppState) -> AppResult<serde_json::Value> {
    to_json(&state.extensions()?.list()?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportExtensionRequest {
    local_path: Option<String>,
    store_url: Option<String>,
    #[serde(default)]
    allow_network: bool,
    #[serde(default)]
    default_enabled: bool,
    #[serde(default)]
    profile_ids: Vec<String>,
}

pub fn import_extension(state: &AppState, request: &Request) -> AppResult<serde_json::Value> {
    let input: ImportExtensionRequest = parse_body(request)?;
    let service = state.extensions()?;
    let extension = match (input.local_path, input.store_url) {
        (Some(local_path), None) => service.import_from_local(ImportExtensionFromLocalInput {
            local_path,
            default_enabled: input.default_enabled,
            profile_ids: input.profile_ids,
        })?,
        (None, Some(store_url)) if input.allow_network => {
            service.import_from_store(ImportExtensionFromStoreInput {
                store_url,
                default_enabled: input.default_enabled,
                profile_ids: input.profile_ids,
            })?
        }
        _ => return Err(ErrorCode::LocalApiBadRequest.into()),
    };
    to_json(&extension)
}

pub fn delete_extension(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    state.extensions()?.delete(id)?;
    Ok(serde_json::json!({ "extensionId": id, "deleted": true }))
}

pub fn launch_profile(state: &AppState, id: &str) -> AppResult<serde_json::Value> {
    let status = tauri::async_runtime::block_on(state.launch_profile_shared(id.to_string()))
        .map_err(|_| AppError::from(ErrorCode::LaunchProcessSpawnFailed))?;
    match status {
        LaunchStatus::Started { .. } => to_json(&status),
        LaunchStatus::Failed { code, .. } => Err(AppError::from(match code.as_str() {
            "storage.locked" => ErrorCode::StorageLocked,
            "storage.databaseWriteFailed" => ErrorCode::StorageDatabaseWriteFailed,
            "launch.profileMissing" => ErrorCode::LaunchProfileMissing,
            "launch.runtimeMissing" => ErrorCode::LaunchRuntimeMissing,
            "launch.runtimeExecutableMissing" => ErrorCode::LaunchRuntimeExecutableMissing,
            "launch.engineBlockedByLocation" => ErrorCode::LaunchEngineBlockedByLocation,
            "launch.processSpawnFailed" => ErrorCode::LaunchProcessSpawnFailed,
            _ => ErrorCode::Unknown,
        })),
    }
}

pub fn launch_temporary_profile(
    state: &AppState,
    request: &Request,
) -> AppResult<serde_json::Value> {
    let input: LaunchTemporaryProfileInput = parse_body(request)?;
    to_json(&tauri::async_runtime::block_on(
        state.launch_temporary_profile_shared(input),
    )?)
}

pub fn launch_custom_temporary_profile(
    state: &AppState,
    request: &Request,
) -> AppResult<serde_json::Value> {
    let input: LaunchCustomTemporaryProfileInput = parse_body(request)?;
    to_json(&tauri::async_runtime::block_on(
        state.launch_custom_temporary_profile_shared(input),
    )?)
}

pub fn close_temporary_profile(state: &AppState, session_id: &str) -> AppResult<serde_json::Value> {
    to_json(&tauri::async_runtime::block_on(
        state.close_temporary_profile_shared(session_id.to_string()),
    )?)
}

pub fn delete_profile_data(
    state: &AppState,
    id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    to_json(&tauri::async_runtime::block_on(
        state.delete_profile_data_shared(id.to_string()),
    )?)
}

pub fn clear_profile_cache(
    state: &AppState,
    id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    to_json(&tauri::async_runtime::block_on(
        state.clear_profile_cache_shared(id.to_string()),
    )?)
}

// ---- Sensitive endpoints (cookies + credentials) --------------------------------

pub fn issue_sensitive_token(
    state: &AppState,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    let input: SensitiveActionInput = parse_body(request)?;
    state.onboarding.verify_sensitive_action(input)?;
    let token = state.sensitive_tokens.issue(key_id);
    Ok(serde_json::json!({ "token": token, "expiresInSeconds": 120 }))
}

fn verify_sensitive(state: &AppState, request: &Request, key_id: &str) -> AppResult<()> {
    if let Some(token) = request.header("x-veil-sensitive-token") {
        return state
            .sensitive_tokens
            .consume(token, key_id)
            .then_some(())
            .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed));
    }
    let input = SensitiveActionInput {
        password: request.header("x-veil-password").map(str::to_string),
        two_factor_code: request.header("x-veil-2fa-code").map(str::to_string),
    };
    state.onboarding.verify_sensitive_action(input)
}

pub fn get_cookies(
    state: &AppState,
    id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    to_json(&state.profiles()?.get_cookies(id)?)
}

pub fn update_cookies(
    state: &AppState,
    id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    let input: UpdateProfileCookiesInput = parse_body(request)?;
    to_json(&state.profiles()?.update_cookies(id, input)?)
}

pub fn list_credentials(
    state: &AppState,
    id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    to_json(&state.profiles()?.list_credentials(id)?)
}

pub fn create_credential(
    state: &AppState,
    id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    let input: UpsertProfileCredentialInput = parse_body(request)?;
    to_json(&state.profiles()?.create_credential(id, input)?)
}

pub fn update_credential(
    state: &AppState,
    id: &str,
    credential_id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    ensure_credential_profile(state, id, credential_id)?;
    let input: UpsertProfileCredentialInput = parse_body(request)?;
    to_json(&state.profiles()?.update_credential(credential_id, input)?)
}

pub fn delete_credential(
    state: &AppState,
    id: &str,
    credential_id: &str,
    request: &Request,
    key_id: &str,
) -> AppResult<serde_json::Value> {
    verify_sensitive(state, request, key_id)?;
    ensure_credential_profile(state, id, credential_id)?;
    state.profiles()?.delete_credential(credential_id)?;
    Ok(serde_json::json!({ "credentialId": credential_id, "deleted": true }))
}

fn ensure_credential_profile(
    state: &AppState,
    profile_id: &str,
    credential_id: &str,
) -> AppResult<()> {
    state.profiles()?.get(profile_id)?;
    let credentials = state.profiles()?.list_credentials(profile_id)?;
    if credentials
        .iter()
        .any(|credential| credential.id == credential_id)
    {
        Ok(())
    } else {
        Err(ErrorCode::LocalApiBadRequest.into())
    }
}

fn parse_body<T: serde::de::DeserializeOwned>(request: &Request) -> AppResult<T> {
    serde_json::from_slice(&request.body).map_err(|_| AppError::from(ErrorCode::LocalApiBadRequest))
}

fn to_json<T: serde::Serialize>(value: &T) -> AppResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|_| AppError::from(ErrorCode::Unknown))
}
