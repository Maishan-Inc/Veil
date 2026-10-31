use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use veil_core::{AppError, AppResult, ErrorCode, Fingerprint, ProfileOs};
use veil_runtime::InstalledRuntime;

use crate::app_state::{AppState, TemporaryEngineProcess};
use crate::commands::ai::finalize_ai_task_start;
use crate::default_page;
use crate::services::{
    CreateGroupInput, CreateProfileInput, DiagnosticAction, DiagnosticComponent, ExtensionService,
    Group, LaunchStatus, Profile, ProfileCookieJar, ProfileCredential, Proxy, ProxyService,
    ProxyTestResult, SensitiveActionInput, StartAiTaskInput, StartAiTaskResult, UpdateGroupInput,
    UpdateProfileCookiesInput, UpsertProfileCredentialInput,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfilesResult {
    pub path: String,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfilesInput {
    pub path: String,
    /// Required for v2 (encrypted) export files; ignored for the legacy v1 plaintext ones.
    pub passphrase: Option<String>,
    /// What to do when an imported profile's name already exists: `skip` the record or
    /// `rename` it with an `(imported)` suffix. IDs are never reused — an import always
    /// creates new rows.
    pub on_conflict: ImportConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportConflict {
    Skip,
    Rename,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfilesResult {
    /// Profiles found in the file, readable or not.
    pub total: usize,
    /// Created rows, including the renamed ones.
    pub imported: usize,
    /// The subset of `imported` whose name collided and got an `(imported)` suffix.
    pub renamed: usize,
    /// Name collisions dropped under the `skip` policy.
    pub skipped: usize,
    /// Records rejected by validation (empty name, incoherent fingerprint, …).
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningProfile {
    pub profile_id: String,
    pub process_id: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRunningState {
    pub profile_id: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProfileDataResult {
    pub profile_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearProfileCacheResult {
    pub profile_id: String,
    pub cleared_paths: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeFingerprintForProxyInput {
    pub proxy_id: String,
    pub os: String,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizedFingerprintForProxyResult {
    pub fingerprint: Fingerprint,
    pub proxy: Proxy,
    pub test: ProxyTestResult,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTemporaryProfileInput {
    pub profile_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchCustomTemporaryProfileInput {
    pub name: String,
    pub proxy_id: Option<String>,
    pub os: String,
    pub platform: String,
    pub browser_version: String,
    pub startup_page: String,
    pub startup_url: String,
    pub fingerprint: Fingerprint,
    pub extension_ids: Vec<String>,
}

impl Default for LaunchCustomTemporaryProfileInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            proxy_id: None,
            os: "windows".to_string(),
            platform: String::new(),
            browser_version: String::new(),
            startup_page: "veil".to_string(),
            startup_url: String::new(),
            fingerprint: Fingerprint::default(),
            extension_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporaryProfileLaunchResult {
    pub session_id: String,
    pub process_id: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseTemporaryProfileResult {
    pub session_id: String,
    pub closed: bool,
    pub data_deleted: bool,
}

/// Sweep the launched-process registry: drop any handles whose underlying engine has
/// exited (e.g. the user closed the browser window) and reset the corresponding profile
/// row to `idle`. Called from `list_profiles` so the UI sees correct status without a
/// background thread.
fn reap_dead_processes(state: &AppState) {
    let dead_ids: Vec<String> = {
        let mut launched = state.launched.lock().expect("launched lock poisoned");
        let mut dead = Vec::new();
        launched.retain(|profile_id, process| {
            if process.is_alive() {
                true
            } else {
                dead.push(profile_id.clone());
                false
            }
        });
        dead
    };
    for id in dead_ids {
        // Best-effort: a stale id may already have been deleted from the database.
        if let Ok(profiles) = state.profiles() {
            let _ = profiles.set_status(&id, "idle", false);
        }
    }
}

fn sync_profile_runtime_statuses(state: &AppState) {
    reap_dead_processes(state);
    let tracked_profile_ids = {
        let launched = state.launched.lock().expect("launched lock poisoned");
        launched.keys().cloned().collect::<BTreeSet<_>>()
    };
    if let Ok(profiles) = state.profiles() {
        let _ = profiles.reset_untracked_active_statuses(&tracked_profile_ids);
    }
}

fn reap_dead_temporary_processes(state: &AppState) {
    let dead: Vec<(String, PathBuf)> = {
        let mut launched = state
            .temporary_launched
            .lock()
            .expect("temporary_launched lock poisoned");
        let mut dead = Vec::new();
        launched.retain(|session_id, entry| {
            if entry.process.is_alive() {
                true
            } else {
                dead.push((session_id.clone(), entry.data_dir.clone()));
                false
            }
        });
        dead
    };
    let root = state.data_dir.join("temp-profiles");
    for (_session_id, data_dir) in dead {
        let _ = remove_dir_under(&root, &data_dir);
    }
}

async fn verify_sensitive(state: &AppState, verification: SensitiveActionInput) -> AppResult<()> {
    let onboarding = state.onboarding.clone();
    tauri::async_runtime::spawn_blocking(move || onboarding.verify_sensitive_action(verification))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> AppResult<Vec<Profile>> {
    sync_profile_runtime_statuses(&state);
    state.profiles()?.list()
}

#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, profile_id: String) -> AppResult<Profile> {
    state.profiles()?.get(&profile_id)
}

#[tauri::command]
pub fn create_profile(state: State<'_, AppState>, input: CreateProfileInput) -> AppResult<Profile> {
    let profile = state.profiles()?.create(input)?;
    state
        .extensions()?
        .assign_defaults_to_profile(&profile.id)?;
    state.profiles()?.get(&profile.id)
}

#[tauri::command]
pub fn update_profile(
    state: State<'_, AppState>,
    profile_id: String,
    input: CreateProfileInput,
) -> AppResult<Profile> {
    state.profiles()?.update(&profile_id, input)
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, profile_id: String) -> AppResult<()> {
    state.profiles()?.delete(&profile_id)
}

#[tauri::command]
pub fn list_running_profiles(state: State<'_, AppState>) -> AppResult<Vec<RunningProfile>> {
    reap_dead_processes(&state);
    let launched = state.launched.lock().expect("launched lock poisoned");
    Ok(launched
        .iter()
        .map(|(profile_id, process)| RunningProfile {
            profile_id: profile_id.clone(),
            process_id: process.id(),
        })
        .collect())
}

#[tauri::command]
pub fn is_profile_running(
    state: State<'_, AppState>,
    profile_id: String,
) -> AppResult<ProfileRunningState> {
    reap_dead_processes(&state);
    let running = state
        .launched
        .lock()
        .expect("launched lock poisoned")
        .contains_key(&profile_id);
    Ok(ProfileRunningState {
        profile_id,
        running,
    })
}

#[tauri::command]
pub async fn export_profiles(
    state: State<'_, AppState>,
    profile_ids: Vec<String>,
    passphrase: String,
    verification: SensitiveActionInput,
) -> AppResult<ExportProfilesResult> {
    if passphrase.trim().is_empty() {
        return Err(AppError::from(ErrorCode::ExportPassphraseRequired));
    }
    let onboarding = state.onboarding.clone();
    let profiles_service = state.profiles()?;
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || onboarding.verify_sensitive_action(verification))
        .await
        .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))??;

    let mut profiles = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for id in profile_ids {
        if seen.insert(id.clone()) {
            profiles.push(profiles_service.get(&id)?);
        }
    }

    let export_dir = data_dir.join("exports");
    let exported_at = now_rfc3339();
    let file_stamp = exported_at.replace([':', '.'], "-");
    let target = export_dir.join(format!("profiles-{file_stamp}.json"));
    let tmp = target.with_extension("json.tmp");
    let envelope = seal_export_envelope(&profiles, &exported_at, &passphrase)?;
    let bytes = serde_json::to_vec_pretty(&envelope)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;

    std::fs::create_dir_all(&export_dir)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
    std::fs::write(&tmp, bytes)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
    std::fs::rename(&tmp, &target)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;

    Ok(ExportProfilesResult {
        path: target.to_string_lossy().to_string(),
        count: profiles.len(),
    })
}

#[tauri::command]
pub async fn import_profiles(
    state: State<'_, AppState>,
    input: ImportProfilesInput,
) -> AppResult<ImportProfilesResult> {
    // Parsing and decryption are CPU-bound but tiny (exports are small JSON files); the
    // create loop below goes through the same locked connection as every other command.
    let path = input.path.clone();
    let (records, unreadable) = tauri::async_runtime::spawn_blocking(move || {
        open_export_file(&path, input.passphrase.as_deref())
    })
    .await
    .map_err(|_| AppError::from(ErrorCode::Unknown))??;

    let profiles_service = state.profiles()?;
    let proxies_service = state.proxies()?;
    let existing_names: std::collections::BTreeSet<String> = profiles_service
        .list()?
        .iter()
        .map(|profile| profile.name.trim().to_lowercase())
        .collect();
    let known_group_ids: std::collections::BTreeSet<String> = profiles_service
        .list_groups()?
        .iter()
        .map(|group| group.id.clone())
        .collect();

    let mut result = ImportProfilesResult {
        total: records.len() + unreadable,
        imported: 0,
        renamed: 0,
        skipped: 0,
        failed: unreadable,
    };
    // Names resolve case-insensitively so "storefront" cannot shadow "Storefront".
    let mut used_names = existing_names;
    for mut record in records {
        let name = record.name.trim().to_string();
        if name.is_empty() {
            result.failed += 1;
            continue;
        }
        if used_names.contains(&name.to_lowercase()) {
            match input.on_conflict {
                ImportConflict::Skip => {
                    result.skipped += 1;
                    continue;
                }
                ImportConflict::Rename => {
                    let renamed = unique_imported_name(&name, &mut used_names);
                    record.name = renamed;
                    result.renamed += 1;
                }
            }
        } else {
            used_names.insert(name.to_lowercase());
        }
        // A foreign proxy / group id belongs to the machine that exported. Keep the
        // assignment only when this library actually has the record; otherwise import the
        // profile unassigned rather than blocking it.
        if let Some(proxy_id) = record.proxy_id.clone() {
            if proxies_service.get(&proxy_id).is_err() {
                record.proxy_id = None;
            }
        }
        if let Some(group_id) = record.group_id.clone() {
            if !known_group_ids.contains(&group_id) {
                record.group_id = None;
            }
        }
        match profiles_service.create(record) {
            Ok(_) => result.imported += 1,
            Err(_) => result.failed += 1,
        }
    }
    Ok(result)
}

const EXPORT_KIND: &str = "veil.profileExport";
/// Legacy plaintext body version, still readable so old exports keep importing.
const EXPORT_VERSION_PLAINTEXT: u64 = 1;
/// The shipped format: the v1 body AES-256-GCM sealed under an Argon2id key derived from
/// the user's passphrase.
const EXPORT_VERSION_ENCRYPTED: u64 = 2;

/// Argon2id cost recorded in the envelope. These mirror `veil_secure_storage::derive_key`'s
/// fixed parameters; changing either side means bumping `EXPORT_VERSION_ENCRYPTED`.
const EXPORT_KDF_ALGO: &str = "argon2id";
const EXPORT_KDF_M_COST_KIB: u32 = 19 * 1024;
const EXPORT_KDF_ITERATIONS: u32 = 2;
const EXPORT_KDF_LANES: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportKdfParams {
    algo: String,
    salt_hex: String,
    m_cost_kib: u32,
    iterations: u32,
    lanes: u32,
}

/// Seal the profile payload into a v2 envelope: the v1 plaintext body, encrypted with
/// AES-256-GCM under a key derived from `passphrase` (Argon2id, per-export random salt).
/// The envelope records the KDF cost so the format stays self-describing.
fn seal_export_envelope(
    profiles: &[Profile],
    exported_at: &str,
    passphrase: &str,
) -> AppResult<serde_json::Value> {
    let body = serde_json::json!({
        "kind": EXPORT_KIND,
        "version": EXPORT_VERSION_PLAINTEXT,
        "exportedAt": exported_at,
        "count": profiles.len(),
        "profiles": profiles,
    });
    let plaintext = serde_json::to_vec(&body)
        .map_err(|_| AppError::from(ErrorCode::StorageEncryptionFailed))?;
    let salt = veil_secure_storage::generate_salt();
    let key = veil_secure_storage::derive_key(passphrase.as_bytes(), &salt)
        .map_err(|_| AppError::from(ErrorCode::StorageEncryptionFailed))?;
    let cipher = veil_secure_storage::wrap_secret_with_dek_hex(&hex::encode(key), &plaintext)
        .map_err(|_| AppError::from(ErrorCode::StorageEncryptionFailed))?;
    Ok(serde_json::json!({
        "kind": EXPORT_KIND,
        "version": EXPORT_VERSION_ENCRYPTED,
        "exportedAt": exported_at,
        "count": profiles.len(),
        "kdf": ExportKdfParams {
            algo: EXPORT_KDF_ALGO.to_string(),
            salt_hex: hex::encode(salt),
            m_cost_kib: EXPORT_KDF_M_COST_KIB,
            iterations: EXPORT_KDF_ITERATIONS,
            lanes: EXPORT_KDF_LANES,
        },
        "cipher": cipher,
    }))
}

/// Read an export file into importable records. Accepts both shipped formats: the v2
/// envelope (passphrase required) and the legacy v1 plaintext body (passphrase ignored).
/// Returns the parsed records plus how many profile entries could not be deserialized.
fn open_export_file(
    path: &str,
    passphrase: Option<&str>,
) -> AppResult<(Vec<CreateProfileInput>, usize)> {
    let raw = std::fs::read_to_string(path)
        .map_err(|_| AppError::from(ErrorCode::ExportFormatInvalid))?;
    open_export_body(&raw, passphrase)
}

fn open_export_body(
    raw: &str,
    passphrase: Option<&str>,
) -> AppResult<(Vec<CreateProfileInput>, usize)> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| AppError::from(ErrorCode::ExportFormatInvalid))?;
    if value.get("kind").and_then(serde_json::Value::as_str) != Some(EXPORT_KIND) {
        return Err(AppError::from(ErrorCode::ExportFormatInvalid));
    }
    let body = match value.get("version").and_then(serde_json::Value::as_u64) {
        Some(EXPORT_VERSION_PLAINTEXT) => value,
        Some(EXPORT_VERSION_ENCRYPTED) => {
            let kdf: ExportKdfParams = serde_json::from_value(
                value
                    .get("kdf")
                    .cloned()
                    .ok_or_else(|| AppError::from(ErrorCode::ExportFormatInvalid))?,
            )
            .map_err(|_| AppError::from(ErrorCode::ExportFormatInvalid))?;
            let cipher: veil_secure_storage::WrappedSecret = serde_json::from_value(
                value
                    .get("cipher")
                    .cloned()
                    .ok_or_else(|| AppError::from(ErrorCode::ExportFormatInvalid))?,
            )
            .map_err(|_| AppError::from(ErrorCode::ExportFormatInvalid))?;
            if kdf.algo != EXPORT_KDF_ALGO {
                return Err(AppError::from(ErrorCode::ExportFormatInvalid));
            }
            let passphrase = passphrase
                .map(str::to_string)
                .filter(|candidate| !candidate.is_empty())
                .ok_or_else(|| AppError::from(ErrorCode::ExportPassphraseRequired))?;
            let salt = hex::decode(&kdf.salt_hex)
                .ok()
                .filter(|salt| salt.len() >= veil_secure_storage::SALT_LEN)
                .ok_or_else(|| AppError::from(ErrorCode::ExportFormatInvalid))?;
            let key = veil_secure_storage::derive_key(passphrase.as_bytes(), &salt)
                .map_err(|_| AppError::from(ErrorCode::StorageDecryptionFailed))?;
            // A wrong passphrase fails here: AES-GCM authentication rejects the plaintext.
            let plaintext =
                veil_secure_storage::unwrap_secret_with_dek_hex(&hex::encode(key), &cipher)
                    .map_err(|_| AppError::from(ErrorCode::StorageDecryptionFailed))?;
            serde_json::from_slice(&plaintext)
                .map_err(|_| AppError::from(ErrorCode::ExportFormatInvalid))?
        }
        _ => return Err(AppError::from(ErrorCode::ExportFormatInvalid)),
    };
    let empty = Vec::new();
    let entries = body
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .unwrap_or(&empty);
    let mut records = Vec::with_capacity(entries.len());
    let mut unreadable = 0usize;
    for entry in entries {
        match serde_json::from_value::<CreateProfileInput>(entry.clone()) {
            Ok(record) => records.push(record),
            Err(_) => unreadable += 1,
        }
    }
    Ok((records, unreadable))
}

/// The first unused `{name} (imported)`, `{name} (imported 2)`, … — registering each pick
/// in `used_names` so two colliding imports cannot resolve to the same name.
fn unique_imported_name(name: &str, used_names: &mut std::collections::BTreeSet<String>) -> String {
    let base = format!("{name} (imported)");
    if used_names.insert(base.to_lowercase()) {
        return base;
    }
    for ordinal in 2.. {
        let candidate = format!("{name} (imported {ordinal})");
        if used_names.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!("ordinal loop always returns")
}

/// Generate a coherent random fingerprint for `os`. `seed` makes it reproducible (the UI
/// passes a fresh seed per click). Pure backend work; no persistence.
#[tauri::command]
pub fn randomize_fingerprint(state: State<'_, AppState>, os: String, seed: u64) -> Fingerprint {
    state
        .profiles()
        .map(|profiles| profiles.randomize_fingerprint(&os, seed))
        .unwrap_or_else(|_| veil_runtime::randomize(&os, seed))
}

/// User-triggered flow: test a configured proxy, persist its detected location, and
/// generate a fingerprint whose language/timezone follows that location. No background
/// networking is involved; the request runs only when the user clicks the optimize action.
#[tauri::command]
pub async fn optimize_fingerprint_for_proxy(
    state: State<'_, AppState>,
    input: OptimizeFingerprintForProxyInput,
) -> AppResult<OptimizedFingerprintForProxyResult> {
    if input.proxy_id.trim().is_empty() {
        return Err(AppError::from(ErrorCode::ProxyNotFound));
    }
    let proxies = state.proxies()?;
    let record = proxies.get_record(&input.proxy_id)?;
    let cfg = veil_runtime::ProxyConfig {
        kind: record.kind,
        host: record.host,
        port: record.port,
        username: record.username,
        password: record.password,
    };
    let outcome = tauri::async_runtime::spawn_blocking(move || veil_runtime::test_proxy(&cfg))
        .await
        .map_err(|_| AppError::from(ErrorCode::Unknown))?
        .map_err(|_| AppError::from(ErrorCode::ProxyTestFailed))?;
    let proxy = proxies.update_location(&input.proxy_id, &outcome)?;
    let fingerprint = veil_runtime::randomize_for_proxy(&input.os, input.seed, &outcome);
    veil_core::validate(&fingerprint, ProfileOs::from_str_or_default(&input.os))?;
    let test = ProxyTestResult::success(outcome);
    Ok(OptimizedFingerprintForProxyResult {
        fingerprint,
        proxy,
        test,
    })
}

/// Validate fingerprint consistency without saving. Used by the dialog's "Validate" check.
///
/// Takes the OS the dialog currently has selected, because three of `validate`'s rules are about the
/// machine the profile claims and the claim is not in the `Fingerprint` (5.5b). Passing it in rather
/// than reading a stored profile is deliberate: the dialog validates what the user is *editing*, and
/// switching the OS from Windows to macOS is exactly the edit that can make a saved-valid display
/// block incoherent.
#[tauri::command]
pub fn validate_fingerprint(
    state: State<'_, AppState>,
    fingerprint: Fingerprint,
    os: String,
) -> AppResult<()> {
    let profile_os = ProfileOs::from_str_or_default(&os);
    state
        .profiles()
        .map(|profiles| profiles.validate_fingerprint(&fingerprint, profile_os))
        .unwrap_or_else(|_| veil_core::validate(&fingerprint, profile_os).map_err(AppError::from))
}

#[tauri::command]
pub fn update_profile_fingerprint(
    state: State<'_, AppState>,
    profile_id: String,
    fingerprint: Fingerprint,
) -> AppResult<Profile> {
    state
        .profiles()?
        .update_fingerprint(&profile_id, fingerprint)
}

#[tauri::command]
pub async fn delete_profile_data(
    state: State<'_, AppState>,
    profile_id: String,
    verification: SensitiveActionInput,
) -> AppResult<DeleteProfileDataResult> {
    verify_sensitive(&state, verification).await?;
    state.delete_profile_data_shared(profile_id).await
}

impl AppState {
    pub(crate) async fn delete_profile_data_shared(
        &self,
        profile_id: String,
    ) -> AppResult<DeleteProfileDataResult> {
        let state = self;
        let profiles = state.profiles()?;
        profiles.get(&profile_id)?;
        if let Some(mut process) = state
            .launched
            .lock()
            .expect("launched lock poisoned")
            .remove(&profile_id)
        {
            let _ = process.stop();
        }
        let _ = profiles.set_status(&profile_id, "idle", false);
        let root = state.data_dir.join("profiles");
        let target = root.join(&profile_id);
        let deleted =
            tauri::async_runtime::spawn_blocking(move || remove_dir_under(&root, &target))
                .await
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        Ok(DeleteProfileDataResult {
            profile_id,
            deleted,
        })
    }
}

#[tauri::command]
pub async fn clear_profile_cache(
    state: State<'_, AppState>,
    profile_id: String,
    verification: SensitiveActionInput,
) -> AppResult<ClearProfileCacheResult> {
    verify_sensitive(&state, verification).await?;
    state.clear_profile_cache_shared(profile_id).await
}

impl AppState {
    pub(crate) async fn clear_profile_cache_shared(
        &self,
        profile_id: String,
    ) -> AppResult<ClearProfileCacheResult> {
        let state = self;
        let profiles = state.profiles()?;
        profiles.get(&profile_id)?;
        if let Some(mut process) = state
            .launched
            .lock()
            .expect("launched lock poisoned")
            .remove(&profile_id)
        {
            let _ = process.stop();
        }
        let _ = profiles.set_status(&profile_id, "idle", false);
        let root = state.data_dir.join("profiles");
        let profile_dir = root.join(&profile_id);
        let cleared_paths =
            tauri::async_runtime::spawn_blocking(move || clear_cache_dirs(&root, &profile_dir))
                .await
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        Ok(ClearProfileCacheResult {
            profile_id,
            cleared_paths,
        })
    }
}

#[tauri::command]
pub async fn get_profile_cookies(
    state: State<'_, AppState>,
    profile_id: String,
    verification: SensitiveActionInput,
) -> AppResult<ProfileCookieJar> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.get_cookies(&profile_id)
}

#[tauri::command]
pub async fn update_profile_cookies(
    state: State<'_, AppState>,
    profile_id: String,
    input: UpdateProfileCookiesInput,
    verification: SensitiveActionInput,
) -> AppResult<ProfileCookieJar> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.update_cookies(&profile_id, input)
}

#[tauri::command]
pub async fn list_profile_credentials(
    state: State<'_, AppState>,
    profile_id: String,
    verification: SensitiveActionInput,
) -> AppResult<Vec<ProfileCredential>> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.list_credentials(&profile_id)
}

#[tauri::command]
pub async fn create_profile_credential(
    state: State<'_, AppState>,
    profile_id: String,
    input: UpsertProfileCredentialInput,
    verification: SensitiveActionInput,
) -> AppResult<ProfileCredential> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.create_credential(&profile_id, input)
}

#[tauri::command]
pub async fn update_profile_credential(
    state: State<'_, AppState>,
    credential_id: String,
    input: UpsertProfileCredentialInput,
    verification: SensitiveActionInput,
) -> AppResult<ProfileCredential> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.update_credential(&credential_id, input)
}

#[tauri::command]
pub async fn delete_profile_credential(
    state: State<'_, AppState>,
    credential_id: String,
    verification: SensitiveActionInput,
) -> AppResult<()> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.delete_credential(&credential_id)
}

/// Real launch path (Phase 1B). Resolves the active engine executable, builds a per-profile
/// isolated `user_data_dir`, spawns the engine through `veil_bridge::launch`, and stores the
/// child process handle in `AppState::launched` so `stop_profile` can terminate it. Errors
/// flow through stable `LaunchStatus::Failed { code }` so the UI can localize via i18n.
#[tauri::command]
pub async fn launch_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<LaunchStatus, ()> {
    let result = state.launch_profile_shared(profile_id).await;
    match &result {
        Ok(LaunchStatus::Started { .. }) => state
            .diagnostics()
            .record_ok(DiagnosticComponent::Launch, DiagnosticAction::Launch),
        Ok(LaunchStatus::Failed { code, .. }) => state.diagnostics().record_failure_code(
            DiagnosticComponent::Launch,
            DiagnosticAction::Launch,
            code,
        ),
        Err(_) => {}
    }
    result
}

impl AppState {
    pub(crate) async fn launch_profile_shared(
        &self,
        profile_id: String,
    ) -> Result<LaunchStatus, ()> {
        let state = self;
        let profiles = match state.profiles() {
            Ok(profiles) => profiles,
            Err(_) => {
                return Ok(LaunchStatus::Failed {
                    profile_id,
                    code: "storage.locked".to_string(),
                });
            }
        };
        let proxies = match state.proxies() {
            Ok(proxies) => proxies,
            Err(_) => {
                return Ok(LaunchStatus::Failed {
                    profile_id,
                    code: "storage.locked".to_string(),
                });
            }
        };
        let extensions = match state.extensions() {
            Ok(extensions) => extensions,
            Err(_) => {
                return Ok(LaunchStatus::Failed {
                    profile_id,
                    code: "storage.locked".to_string(),
                });
            }
        };

        // Profile must exist — surfaced as a failed status so the row shows the error.
        let profile = match profiles.get(&profile_id) {
            Ok(p) => p,
            Err(_) => {
                return Ok(LaunchStatus::Failed {
                    profile_id,
                    code: "launch.profileMissing".to_string(),
                });
            }
        };

        let Some(runtime) = state.runtime.resolve_runtime(&profile.browser_version) else {
            return Ok(LaunchStatus::Failed {
                profile_id: profile.id,
                code: "launch.runtimeMissing".to_string(),
            });
        };
        // Read off the engine that is about to start, not off `engine.toml`. One engine, one dialect, one
        // source — the same rule `--fingerprint-platform` follows for the OS (5.5b). A manifest-global
        // dialect hands the patched vocabulary to whatever happens to be installed (5.5c).
        let flag_dialect = runtime.effective_dialect();
        let engine_version = runtime.version;
        let exe = runtime.executable_path;
        if !exe.exists() {
            return Ok(LaunchStatus::Failed {
                profile_id: profile.id,
                code: "launch.runtimeExecutableMissing".to_string(),
            });
        }

        let user_data_dir = state.data_dir.join("profiles").join(&profile.id);
        if let Err(_err) = std::fs::create_dir_all(&user_data_dir) {
            return Ok(LaunchStatus::Failed {
                profile_id: profile.id,
                code: "storage.databaseWriteFailed".to_string(),
            });
        }

        // Resolve proxy → engine flag. A missing / deleted proxy id silently maps to "no
        // proxy" rather than blocking the launch; the foreign-key in `profiles.proxy_id` has
        // ON DELETE SET NULL but a launch in flight after a delete could still race.
        //
        // Chromium's --proxy-server does NOT accept embedded credentials. For the v0.x line
        // we launch without authenticated proxies and document this in the UI; a future
        // phase will inject auth via a sideloaded extension. SOCKS5 needs the explicit
        // scheme so Chromium uses CONNECT rather than the default HTTP handling.
        // On a `veil-patched` engine the last-tested exit IP also rides along as
        // `--webrtc-ip=` — see `push_proxy_arg` for the full contract; the two blocks are the
        // same rule written once per launch path.
        let mut extra_args = match profile.proxy_id.as_deref() {
            Some(proxy_id) => match proxies.get(proxy_id) {
                Ok(p) => {
                    let mut args = vec![format!(
                        "--proxy-server={}://{}:{}",
                        p.proxy_type, p.host, p.port
                    )];
                    if matches!(flag_dialect, veil_runtime::FlagDialect::VeilPatched) {
                        if let Some(exit_ip) =
                            p.detected_ip.as_deref().filter(|ip| !ip.trim().is_empty())
                        {
                            args.push(format!("--webrtc-ip={exit_ip}"));
                        }
                    }
                    args
                }
                Err(_) => Vec::new(),
            },
            None => Vec::new(),
        };
        // Fingerprint config → engine flags. Veil only assembles the flags; the engine applies
        // them. Empty/zero fields produce nothing, so an unconfigured profile is unaffected. The
        // launching engine's own flag dialect decides which switches may be emitted at all — anything
        // it does not implement is dropped rather than silently ignored by Chromium.
        let launch_options = veil_core::to_engine_args(
            &profile.fingerprint,
            veil_core::ProfileOs::from_str_or_default(&profile.os),
            flag_dialect,
        );
        extra_args.extend(launch_options.args);
        if let Ok(extension_paths) = extensions.installed_for_profile(&profile.id) {
            if !extension_paths.is_empty() {
                let joined = extension_paths
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                extra_args.push(format!("--disable-extensions-except={joined}"));
                extra_args.push(format!("--load-extension={joined}"));
            }
        }
        match default_page::resolve_startup_url(
            &state.data_dir,
            &profile.startup_page,
            &profile.startup_url,
            &engine_version,
        ) {
            Ok(url) => extra_args.push(url),
            Err(_) => {
                return Ok(LaunchStatus::Failed {
                    profile_id: profile.id,
                    code: "storage.databaseWriteFailed".to_string(),
                });
            }
        }

        let request = veil_bridge::LaunchRequest {
            executable_path: exe,
            user_data_dir,
            profile_id: profile.id.clone(),
            extra_args,
            env: launch_options.env,
        };

        // bridge::launch spawns a child process — keep it off the async runtime.
        let spawn = tauri::async_runtime::spawn_blocking(move || veil_bridge::launch(&request))
            .await
            .map_err(|_| ())?;

        let (process, result) = match spawn {
            Ok(pair) => pair,
            Err(err) => {
                let code = match err {
                    veil_bridge::BridgeError::ExecutableMissing => {
                        "launch.runtimeExecutableMissing"
                    }
                    veil_bridge::BridgeError::ActivationContextFailed => {
                        "launch.engineBlockedByLocation"
                    }
                    veil_bridge::BridgeError::Spawn(_) => "launch.processSpawnFailed",
                };
                return Ok(LaunchStatus::Failed {
                    profile_id: profile.id,
                    code: code.to_string(),
                });
            }
        };

        // If a previous launch is still tracked, drop it (best-effort kill).
        {
            let mut launched = state.launched.lock().expect("launched lock poisoned");
            if let Some(mut prev) = launched.insert(profile.id.clone(), process) {
                let _ = prev.stop();
            }
        }

        // Best-effort status flip; even if persistence fails we still report Started.
        let _ = profiles.set_status(&profile.id, "running", true);

        Ok(LaunchStatus::Started {
            profile_id: profile.id,
            process_id: result.process_id,
        })
    }
}

#[tauri::command]
pub async fn launch_temporary_profile(
    state: State<'_, AppState>,
    input: LaunchTemporaryProfileInput,
) -> AppResult<TemporaryProfileLaunchResult> {
    state.launch_temporary_profile_shared(input).await
}

impl AppState {
    pub(crate) async fn launch_temporary_profile_shared(
        &self,
        input: LaunchTemporaryProfileInput,
    ) -> AppResult<TemporaryProfileLaunchResult> {
        let state = self;
        reap_dead_temporary_processes(state);
        let profiles = state.profiles()?;
        let proxies = state.proxies()?;
        let extensions = state.extensions()?;
        let profile = profiles.get(&input.profile_id)?;
        // Resolved before the flags are built, not after: the dialect and the version label must come off
        // the same entry `spawn_temporary_browser` will resolve again by version. An unresolvable engine
        // leaves the dialect at its conservative default — nothing launches in that case anyway.
        let runtime = state.runtime.resolve_runtime(&profile.browser_version);
        let flag_dialect = runtime
            .as_ref()
            .map(InstalledRuntime::effective_dialect)
            .unwrap_or_default();
        let mut extra_args = Vec::new();
        push_proxy_arg(
            &mut extra_args,
            &proxies,
            profile.proxy_id.as_deref(),
            false,
            flag_dialect,
        )?;
        let launch_options = veil_core::to_engine_args(
            &profile.fingerprint,
            veil_core::ProfileOs::from_str_or_default(&profile.os),
            flag_dialect,
        );
        extra_args.extend(launch_options.args);
        push_extension_args(
            &mut extra_args,
            extensions.installed_for_profile(&profile.id)?,
        );
        let session_id = temporary_session_id();
        let user_data_dir = state.data_dir.join("temp-profiles").join(&session_id);
        let engine_version = runtime
            .map(|runtime| runtime.version)
            .unwrap_or_else(|| profile.browser_version.clone());
        let url = default_page::resolve_startup_url(
            &state.data_dir,
            &profile.startup_page,
            &profile.startup_url,
            &engine_version,
        )
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        extra_args.push(url);
        spawn_temporary_browser(
            state,
            session_id,
            profile.browser_version,
            user_data_dir,
            extra_args,
            launch_options.env,
        )
        .await
    }
}

#[tauri::command]
pub async fn launch_custom_temporary_profile(
    state: State<'_, AppState>,
    input: LaunchCustomTemporaryProfileInput,
) -> AppResult<TemporaryProfileLaunchResult> {
    state.launch_custom_temporary_profile_shared(input).await
}

impl AppState {
    pub(crate) async fn launch_custom_temporary_profile_shared(
        &self,
        input: LaunchCustomTemporaryProfileInput,
    ) -> AppResult<TemporaryProfileLaunchResult> {
        let state = self;
        reap_dead_temporary_processes(state);
        veil_core::validate(
            &input.fingerprint,
            ProfileOs::from_str_or_default(&input.os),
        )?;
        let proxies = state.proxies()?;
        let extensions = state.extensions()?;
        let browser_version = input.browser_version.trim().to_string();
        let runtime = state.runtime.resolve_runtime(&browser_version);
        let mut extra_args = Vec::new();
        push_proxy_arg(
            &mut extra_args,
            &proxies,
            input.proxy_id.as_deref(),
            true,
            runtime
                .as_ref()
                .map(InstalledRuntime::effective_dialect)
                .unwrap_or_default(),
        )?;
        // Preview launches get the same dialect gate as a real one, read off the same engine: a preview
        // that reports itself as spoofed while running on a stock engine is the same lie.
        let launch_options = veil_core::to_engine_args(
            &input.fingerprint,
            veil_core::ProfileOs::from_str_or_default(&input.os),
            runtime
                .as_ref()
                .map(InstalledRuntime::effective_dialect)
                .unwrap_or_default(),
        );
        extra_args.extend(launch_options.args);
        push_extension_args(
            &mut extra_args,
            extension_paths_by_id(&extensions, input.extension_ids)?,
        );
        let session_id = temporary_session_id();
        let user_data_dir = state.data_dir.join("temp-profiles").join(&session_id);
        let engine_version = runtime
            .map(|runtime| runtime.version)
            .unwrap_or_else(|| browser_version.clone());
        let startup_page = normalize_temporary_startup_page(&input.startup_page);
        let url = default_page::resolve_startup_url(
            &state.data_dir,
            &startup_page,
            &input.startup_url,
            &engine_version,
        )
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        extra_args.push(url);
        spawn_temporary_browser(
            state,
            session_id,
            browser_version,
            user_data_dir,
            extra_args,
            launch_options.env,
        )
        .await
    }
}

#[tauri::command]
pub async fn close_temporary_profile(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<CloseTemporaryProfileResult> {
    state.close_temporary_profile_shared(session_id).await
}

impl AppState {
    pub(crate) async fn close_temporary_profile_shared(
        &self,
        session_id: String,
    ) -> AppResult<CloseTemporaryProfileResult> {
        let state = self;
        let entry = state
            .temporary_launched
            .lock()
            .expect("temporary_launched lock poisoned")
            .remove(&session_id);
        let Some(mut entry) = entry else {
            return Ok(CloseTemporaryProfileResult {
                session_id,
                closed: false,
                data_deleted: false,
            });
        };
        let _ = entry.process.stop();
        let root = state.data_dir.join("temp-profiles");
        let data_dir = entry.data_dir;
        let data_deleted =
            tauri::async_runtime::spawn_blocking(move || remove_dir_under(&root, &data_dir))
                .await
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        Ok(CloseTemporaryProfileResult {
            session_id,
            closed: true,
            data_deleted,
        })
    }
}

#[tauri::command]
pub async fn launch_profile_for_ai(
    state: State<'_, AppState>,
    input: StartAiTaskInput,
) -> AppResult<StartAiTaskResult> {
    let ai = state.ai()?;
    let has_provider = ai.has_provider_credentials_for(input.model_config_id.as_deref())?;
    if !has_provider {
        return Err(AppError::from(ErrorCode::AiProviderMissing));
    }
    let profiles = state.profiles()?;
    let proxies = state.proxies()?;
    let extensions = state.extensions()?;
    let profile = profiles.get(&input.profile_id)?;
    let Some(runtime) = state.runtime.resolve_runtime(&profile.browser_version) else {
        return Err(AppError::from(ErrorCode::LaunchRuntimeMissing));
    };
    let flag_dialect = runtime.effective_dialect();
    let engine_version = runtime.version;
    let exe = runtime.executable_path;
    if !exe.exists() {
        return Err(AppError::from(ErrorCode::LaunchRuntimeExecutableMissing));
    }
    let user_data_dir = state.data_dir.join("profiles").join(&profile.id);
    std::fs::create_dir_all(&user_data_dir)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;

    let debug_port = ai.allocate_debug_port()?;
    let mut extra_args = vec![
        format!("--remote-debugging-port={debug_port}"),
        "--remote-debugging-address=127.0.0.1".to_string(),
    ];
    if let Some(proxy_id) = profile.proxy_id.as_deref() {
        if let Ok(p) = proxies.get(proxy_id) {
            extra_args.push(format!(
                "--proxy-server={}://{}:{}",
                p.proxy_type, p.host, p.port
            ));
            if matches!(flag_dialect, veil_runtime::FlagDialect::VeilPatched) {
                if let Some(exit_ip) = p.detected_ip.as_deref().filter(|ip| !ip.trim().is_empty()) {
                    extra_args.push(format!("--webrtc-ip={exit_ip}"));
                }
            }
        }
    }
    // The automation path is gated on the same dialect, read off the same engine: an AI session must
    // not be handed a profile the UI called spoofed when the engine ignored the switches.
    let launch_options = veil_core::to_engine_args(
        &profile.fingerprint,
        veil_core::ProfileOs::from_str_or_default(&profile.os),
        flag_dialect,
    );
    extra_args.extend(launch_options.args);
    let extension_paths = extensions.installed_for_profile(&profile.id)?;
    if !extension_paths.is_empty() {
        let joined = extension_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(",");
        extra_args.push(format!("--disable-extensions-except={joined}"));
        extra_args.push(format!("--load-extension={joined}"));
    }
    let url = default_page::resolve_startup_url(
        &state.data_dir,
        &profile.startup_page,
        &profile.startup_url,
        &engine_version,
    )
    .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
    extra_args.push(url);

    let request = veil_bridge::LaunchRequest {
        executable_path: exe,
        user_data_dir,
        profile_id: profile.id.clone(),
        extra_args,
        env: launch_options.env,
    };
    let spawn = tauri::async_runtime::spawn_blocking(move || veil_bridge::launch(&request))
        .await
        .map_err(|_| AppError::from(ErrorCode::LaunchProcessSpawnFailed))?;
    let (process, result) = match spawn {
        Ok(pair) => pair,
        Err(err) => {
            return Err(AppError::from(match err {
                veil_bridge::BridgeError::ExecutableMissing => {
                    ErrorCode::LaunchRuntimeExecutableMissing
                }
                veil_bridge::BridgeError::ActivationContextFailed => {
                    ErrorCode::LaunchEngineBlockedByLocation
                }
                veil_bridge::BridgeError::Spawn(_) => ErrorCode::LaunchProcessSpawnFailed,
            }));
        }
    };

    {
        let mut launched = state.launched.lock().expect("launched lock poisoned");
        if let Some(mut prev) = launched.insert(profile.id.clone(), process) {
            let _ = prev.stop();
        }
    }
    let _ = profiles.set_status(&profile.id, "running", true);
    let session = state
        .ai()?
        .create_session(profile.id, result.process_id, debug_port);
    finalize_ai_task_start(&state, input, session)
}

#[tauri::command]
pub fn stop_profile(state: State<'_, AppState>, profile_id: String) -> AppResult<Profile> {
    let result = state.stop_profile_shared(profile_id);
    match &result {
        Ok(_) => state
            .diagnostics()
            .record_ok(DiagnosticComponent::Launch, DiagnosticAction::Stop),
        Err(err) => state.diagnostics().record_failure(
            DiagnosticComponent::Launch,
            DiagnosticAction::Stop,
            err.code(),
        ),
    }
    result
}

impl AppState {
    pub(crate) fn stop_profile_shared(&self, profile_id: String) -> AppResult<Profile> {
        let state = self;
        // Best-effort terminate any tracked engine process.
        if let Some(mut process) = state
            .launched
            .lock()
            .expect("launched lock poisoned")
            .remove(&profile_id)
        {
            let _ = process.stop();
        }
        state.profiles()?.set_status(&profile_id, "idle", false)
    }
}

#[tauri::command]
pub fn list_groups(state: State<'_, AppState>) -> AppResult<Vec<Group>> {
    state.profiles()?.list_groups()
}

#[tauri::command]
pub fn create_group(state: State<'_, AppState>, input: CreateGroupInput) -> AppResult<Group> {
    state.profiles()?.create_group(input)
}

#[tauri::command]
pub fn update_group(
    state: State<'_, AppState>,
    group_id: String,
    input: UpdateGroupInput,
) -> AppResult<Group> {
    state.profiles()?.update_group(&group_id, input)
}

#[tauri::command]
pub async fn delete_group(
    state: State<'_, AppState>,
    group_id: String,
    verification: SensitiveActionInput,
) -> AppResult<()> {
    verify_sensitive(&state, verification).await?;
    state.profiles()?.delete_group(&group_id)
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

async fn spawn_temporary_browser(
    state: &AppState,
    session_id: String,
    browser_version: String,
    user_data_dir: PathBuf,
    extra_args: Vec<String>,
    env: Vec<(String, String)>,
) -> AppResult<TemporaryProfileLaunchResult> {
    let Some(runtime) = state.runtime.resolve_runtime(&browser_version) else {
        return Err(AppError::from(ErrorCode::LaunchRuntimeMissing));
    };
    let exe = runtime.executable_path;
    if !exe.exists() {
        return Err(AppError::from(ErrorCode::LaunchRuntimeExecutableMissing));
    }
    std::fs::create_dir_all(&user_data_dir)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
    let request = veil_bridge::LaunchRequest {
        executable_path: exe,
        user_data_dir: user_data_dir.clone(),
        profile_id: session_id.clone(),
        extra_args,
        env,
    };
    let spawn = tauri::async_runtime::spawn_blocking(move || veil_bridge::launch(&request))
        .await
        .map_err(|_| AppError::from(ErrorCode::LaunchProcessSpawnFailed))?;
    let (process, result) = match spawn {
        Ok(pair) => pair,
        Err(err) => {
            let _ = remove_dir_under(&user_data_dir, &user_data_dir);
            return Err(AppError::from(match err {
                veil_bridge::BridgeError::ExecutableMissing => {
                    ErrorCode::LaunchRuntimeExecutableMissing
                }
                veil_bridge::BridgeError::ActivationContextFailed => {
                    ErrorCode::LaunchEngineBlockedByLocation
                }
                veil_bridge::BridgeError::Spawn(_) => ErrorCode::LaunchProcessSpawnFailed,
            }));
        }
    };
    state
        .temporary_launched
        .lock()
        .expect("temporary_launched lock poisoned")
        .insert(
            session_id.clone(),
            TemporaryEngineProcess {
                process,
                data_dir: user_data_dir,
            },
        );
    Ok(TemporaryProfileLaunchResult {
        session_id,
        process_id: result.process_id,
    })
}

/// Resolve proxy → engine flags. A missing / deleted proxy id silently maps to "no proxy"
/// rather than blocking the launch unless `strict` (the preview path, which has the row in
/// hand); the foreign-key in `profiles.proxy_id` has ON DELETE SET NULL but a launch in flight
/// after a delete could still race.
///
/// Chromium's --proxy-server does NOT accept embedded credentials. For the v0.x line
/// we launch without authenticated proxies and document this in the UI; a future
/// phase will inject auth via a sideloaded extension. SOCKS5 needs the explicit
/// scheme so Chromium uses CONNECT rather than the default HTTP handling.
///
/// On a `veil-patched` engine the proxy's last-tested exit IP rides along as
/// `--webrtc-ip=` (Veil-Chromium `983`, ported from clearcote's `100`): the engine fabricates
/// the WebRTC server-reflexive candidate at that IP, so a page reading candidates sees the
/// same address its HTTP traffic comes from. The value is the proxy's `detected_ip` — the
/// mirror is this proxy UI, not a `Fingerprint` field (6.9). A proxy never tested, or a stale
/// record, emits nothing: a wrong pinned IP is worse than none, and the policy switch the
/// profile already carries still governs what is gathered. `--webrtc-ip` is not one of `000`'s
/// twelve and stock Chromium has no such switch, so the dialect gate is what keeps it honest.
fn push_proxy_arg(
    extra_args: &mut Vec<String>,
    proxies: &ProxyService,
    proxy_id: Option<&str>,
    strict: bool,
    dialect: veil_runtime::FlagDialect,
) -> AppResult<()> {
    let Some(proxy_id) = proxy_id else {
        return Ok(());
    };
    match proxies.get(proxy_id) {
        Ok(proxy) => {
            extra_args.push(format!(
                "--proxy-server={}://{}:{}",
                proxy.proxy_type, proxy.host, proxy.port
            ));
            if matches!(dialect, veil_runtime::FlagDialect::VeilPatched) {
                if let Some(exit_ip) = proxy
                    .detected_ip
                    .as_deref()
                    .filter(|ip| !ip.trim().is_empty())
                {
                    extra_args.push(format!("--webrtc-ip={exit_ip}"));
                }
            }
            Ok(())
        }
        Err(err) if strict => Err(err),
        Err(_) => Ok(()),
    }
}

fn push_extension_args(extra_args: &mut Vec<String>, extension_paths: Vec<PathBuf>) {
    if extension_paths.is_empty() {
        return;
    }
    let joined = extension_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(",");
    extra_args.push(format!("--disable-extensions-except={joined}"));
    extra_args.push(format!("--load-extension={joined}"));
}

fn extension_paths_by_id(
    extensions: &ExtensionService,
    extension_ids: Vec<String>,
) -> AppResult<Vec<PathBuf>> {
    let mut seen = std::collections::BTreeSet::new();
    let mut paths = Vec::new();
    for id in extension_ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let extension = extensions.get(&id)?;
        let path = PathBuf::from(extension.install_path);
        if path.is_dir() {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn temporary_session_id() -> String {
    format!("temp_{}", Uuid::new_v4())
}

fn normalize_temporary_startup_page(value: &str) -> String {
    match value {
        "google" | "bing" | "custom" => value.to_string(),
        _ => "veil".to_string(),
    }
}

fn remove_dir_under(root: &Path, target: &Path) -> std::io::Result<bool> {
    if !target.exists() {
        return Ok(false);
    }
    let root = canonicalize_existing_or_self(root)?;
    let target = target.canonicalize()?;
    if !target.starts_with(&root) {
        return Ok(false);
    }
    std::fs::remove_dir_all(target)?;
    Ok(true)
}

fn clear_cache_dirs(root: &Path, profile_dir: &Path) -> std::io::Result<usize> {
    let cache_dirs = [
        profile_dir.join("Default").join("Cache"),
        profile_dir.join("Default").join("Code Cache"),
        profile_dir.join("Default").join("GPUCache"),
        profile_dir.join("Cache"),
        profile_dir.join("Code Cache"),
        profile_dir.join("GPUCache"),
        profile_dir.join("ShaderCache"),
    ];
    let mut cleared = 0usize;
    for cache_dir in cache_dirs {
        if remove_dir_under(root, &cache_dir)? {
            cleared += 1;
        }
    }
    Ok(cleared)
}

fn canonicalize_existing_or_self(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        path.canonicalize()
    } else {
        Ok(path.to_path_buf())
    }
}

#[cfg(test)]
mod profile_export_tests {
    use super::*;
    use veil_core::Fingerprint;

    fn sample_profile(name: &str) -> Profile {
        Profile {
            id: format!("id-{name}"),
            name: name.to_string(),
            avatar_color: "#5640c9".to_string(),
            group_id: None,
            tag_ids: Vec::new(),
            proxy_id: None,
            os: "windows".to_string(),
            platform: "Storefront".to_string(),
            browser_version: String::new(),
            notes: String::new(),
            status: "idle".to_string(),
            startup_page: "veil".to_string(),
            startup_url: String::new(),
            fingerprint: Fingerprint::default(),
            created_at: "2026-09-06T00:00:00Z".to_string(),
            updated_at: "2026-09-06T00:00:00Z".to_string(),
            last_launched_at: None,
        }
    }

    #[test]
    fn sealed_envelope_round_trips_through_the_passphrase() {
        let profiles = [sample_profile("Alpha"), sample_profile("Bravo")];
        let envelope =
            seal_export_envelope(&profiles, "2026-09-06T00:00:00Z", "correct horse").unwrap();
        assert_eq!(
            envelope["version"],
            serde_json::json!(EXPORT_VERSION_ENCRYPTED)
        );
        assert!(envelope["cipher"]["ciphertextHex"].is_string());
        assert!(envelope.get("profiles").is_none());

        let raw = serde_json::to_string(&envelope).unwrap();
        let (records, unreadable) = open_export_body(&raw, Some("correct horse")).unwrap();
        assert_eq!(unreadable, 0);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "Alpha");
        assert_eq!(records[0].startup_page, "veil");
    }

    #[test]
    fn wrong_passphrase_fails_authentication_not_parsing() {
        let profiles = [sample_profile("Alpha")];
        let envelope = seal_export_envelope(&profiles, "t", "right").unwrap();
        let raw = serde_json::to_string(&envelope).unwrap();
        let err = open_export_body(&raw, Some("wrong")).unwrap_err();
        assert_eq!(err.code(), ErrorCode::StorageDecryptionFailed);
    }

    #[test]
    fn encrypted_export_without_a_passphrase_is_rejected() {
        let profiles = [sample_profile("Alpha")];
        let envelope = seal_export_envelope(&profiles, "t", "right").unwrap();
        let raw = serde_json::to_string(&envelope).unwrap();
        let err = open_export_body(&raw, None).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ExportPassphraseRequired);
    }

    #[test]
    fn legacy_v1_plaintext_still_imports_without_a_passphrase() {
        let body = serde_json::json!({
            "kind": EXPORT_KIND,
            "version": 1,
            "exportedAt": "t",
            "count": 1,
            "profiles": [{ "name": "Legacy", "os": "windows" }],
        });
        let raw = serde_json::to_string(&body).unwrap();
        let (records, unreadable) = open_export_body(&raw, None).unwrap();
        assert_eq!(unreadable, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "Legacy");
    }

    #[test]
    fn foreign_and_corrupt_bodies_are_format_errors() {
        let err = open_export_body("not json", None).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ExportFormatInvalid);
        let err = open_export_body("{\"kind\": \"other\", \"version\": 2}", None).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ExportFormatInvalid);
        let err = open_export_body("{\"kind\": \"veil.profileExport\", \"version\": 99}", None)
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::ExportFormatInvalid);
    }

    #[test]
    fn unreadable_profile_entries_are_counted_not_fatal() {
        let body = serde_json::json!({
            "kind": EXPORT_KIND,
            "version": 1,
            "profiles": [{ "name": "Fine", "os": "windows" }, { "fingerprint": 42 }],
        });
        let raw = serde_json::to_string(&body).unwrap();
        let (records, unreadable) = open_export_body(&raw, None).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(unreadable, 1);
    }

    #[test]
    fn imported_names_never_collide() {
        let mut used: std::collections::BTreeSet<String> =
            ["alpha"].iter().map(|name| name.to_string()).collect();
        assert_eq!(unique_imported_name("Alpha", &mut used), "Alpha (imported)");
        assert_eq!(
            unique_imported_name("Alpha", &mut used),
            "Alpha (imported 2)"
        );
        // A free name is used as-is; the renamed variant is registered so the next
        // colliding record cannot resolve to the same string.
        let taken = unique_imported_name("Bravo", &mut used);
        assert_eq!(taken, "Bravo (imported)");
        assert!(used.contains("bravo (imported)"));
    }
}

#[cfg(test)]
mod webrtc_ip_tests {
    use super::*;
    use crate::services::CreateProxyInput;
    use std::sync::{Arc, Mutex};

    fn proxy_service() -> ProxyService {
        let db = veil_database::Database::open_in_memory("test-key").unwrap();
        ProxyService::new(Arc::new(Mutex::new(db)))
    }

    fn create_test_proxy(svc: &ProxyService, exit_ip: Option<&str>) -> Proxy {
        let proxy = svc
            .create(CreateProxyInput {
                name: "T19".into(),
                proxy_type: "http".into(),
                host: "203.0.113.7".into(),
                port: 8080,
                username: None,
                password: None,
                country: None,
            })
            .unwrap();
        if let Some(ip) = exit_ip {
            svc.update_location(
                &proxy.id,
                &veil_runtime::ProxyTestOutcome {
                    ip: Some(ip.into()),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        svc.get(&proxy.id).unwrap()
    }

    fn persisted_id(svc: &ProxyService, proxy: &Proxy) -> String {
        // push_proxy_arg reads by id, so the row must persist past the DTO.
        let _ = svc.get_record(&proxy.id).unwrap();
        proxy.id.clone()
    }

    #[test]
    fn webrtc_ip_follows_the_proxy_exit_on_a_patched_engine() {
        let svc = proxy_service();
        let proxy = create_test_proxy(&svc, Some("203.0.113.99"));
        let id = persisted_id(&svc, &proxy);
        let mut args = Vec::new();
        push_proxy_arg(
            &mut args,
            &svc,
            Some(&id),
            false,
            veil_runtime::FlagDialect::VeilPatched,
        )
        .unwrap();
        assert!(args.contains(&"--proxy-server=http://203.0.113.7:8080".to_string()));
        // 983's whole point: the candidate carries the same address the page's HTTP comes from.
        assert!(args.contains(&"--webrtc-ip=203.0.113.99".to_string()));
    }

    #[test]
    fn stock_dialect_never_emits_webrtc_ip() {
        let svc = proxy_service();
        let proxy = create_test_proxy(&svc, Some("203.0.113.99"));
        let id = persisted_id(&svc, &proxy);
        let mut args = Vec::new();
        push_proxy_arg(
            &mut args,
            &svc,
            Some(&id),
            false,
            veil_runtime::FlagDialect::StockChromium,
        )
        .unwrap();
        assert!(args.iter().all(|a| !a.starts_with("--webrtc-ip=")));
    }

    #[test]
    fn an_untested_proxy_emits_no_webrtc_ip() {
        // A stale or absent `detected_ip` pins a wrong address in the engine's fabricator —
        // strictly worse than none, and the profile's policy switch still governs gathering.
        let svc = proxy_service();
        let proxy = create_test_proxy(&svc, None);
        let id = persisted_id(&svc, &proxy);
        let mut args = Vec::new();
        push_proxy_arg(
            &mut args,
            &svc,
            Some(&id),
            false,
            veil_runtime::FlagDialect::VeilPatched,
        )
        .unwrap();
        assert!(args.iter().all(|a| !a.starts_with("--webrtc-ip=")));
    }
}
