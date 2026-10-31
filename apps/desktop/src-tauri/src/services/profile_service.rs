use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use veil_core::{AppError, AppResult, ErrorCode, Fingerprint, ProfileOs};
use veil_database::{
    CreateGroup, CreateProfile, CreateProfileCredential, Database, DatabaseError, GroupRecord,
    ProfileCredentialRecord, ProfileRecord, UpdateGroup, UpdateProfile, UpdateProfileCredential,
};

/// Profile DTO sent across the IPC boundary. Stays camelCase via serde so the TypeScript
/// `Profile` contract in `@veil/shared` matches without manual mapping.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub avatar_color: String,
    pub group_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub proxy_id: Option<String>,
    pub os: String,
    pub platform: String,
    pub browser_version: String,
    pub notes: String,
    pub status: String,
    pub startup_page: String,
    pub startup_url: String,
    pub fingerprint: Fingerprint,
    pub created_at: String,
    pub updated_at: String,
    pub last_launched_at: Option<String>,
}

impl From<ProfileRecord> for Profile {
    fn from(record: ProfileRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            avatar_color: record.avatar_color,
            group_id: record.group_id,
            tag_ids: record.tag_ids,
            proxy_id: record.proxy_id,
            os: record.os,
            platform: record.platform,
            browser_version: record.browser_version,
            notes: record.notes,
            status: record.status,
            startup_page: record.startup_page,
            startup_url: record.startup_url,
            fingerprint: record.fingerprint,
            created_at: record.created_at,
            updated_at: record.updated_at,
            last_launched_at: record.last_launched_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateProfileInput {
    pub name: String,
    pub group_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub proxy_id: Option<String>,
    pub os: String,
    pub platform: String,
    /// Optional. When empty, launch resolution falls back to the current installed runtime.
    pub browser_version: String,
    pub notes: String,
    #[serde(default = "default_startup_page")]
    pub startup_page: String,
    pub startup_url: String,
    #[serde(default)]
    pub fingerprint: Fingerprint,
}

impl Default for CreateProfileInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            group_id: None,
            tag_ids: Vec::new(),
            proxy_id: None,
            os: "windows".to_string(),
            platform: String::new(),
            browser_version: String::new(),
            notes: String::new(),
            startup_page: default_startup_page(),
            startup_url: String::new(),
            fingerprint: Fingerprint::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GroupRecord> for Group {
    fn from(record: GroupRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            color: record.color,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateGroupInput {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateGroupInput {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCookieJar {
    pub profile_id: String,
    pub cookies: serde_json::Value,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileCookiesInput {
    pub cookies: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCredential {
    pub id: String,
    pub profile_id: String,
    pub platform: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ProfileCredentialRecord> for ProfileCredential {
    fn from(record: ProfileCredentialRecord) -> Self {
        Self {
            id: record.id,
            profile_id: record.profile_id,
            platform: record.platform,
            username: record.username,
            password: record.password,
            notes: record.notes,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UpsertProfileCredentialInput {
    pub platform: String,
    pub username: String,
    pub password: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[allow(dead_code)]
pub enum LaunchStatus {
    #[serde(rename_all = "camelCase")]
    Started { profile_id: String, process_id: u32 },
    #[serde(rename_all = "camelCase")]
    Failed { profile_id: String, code: String },
}

/// SQLCipher-backed profile library.
///
/// Wraps a single `rusqlite::Connection` behind `Arc<Mutex<Database>>` so it can be cloned
/// into Tauri command handlers and short-running async tasks without breaking `Send + Sync`
/// (rusqlite's `Connection` is `Send` but not `Sync`).
#[derive(Clone)]
pub struct ProfileService {
    db: Arc<Mutex<Database>>,
}

impl ProfileService {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Database> {
        self.db.lock().expect("database lock poisoned")
    }

    pub fn list(&self) -> AppResult<Vec<Profile>> {
        let records = self.db().list_profiles().map_err(map_read_error)?;
        Ok(records.into_iter().map(Profile::from).collect())
    }

    pub fn get(&self, id: &str) -> AppResult<Profile> {
        let record = self.db().get_profile(id).map_err(map_read_error)?;
        Ok(Profile::from(record))
    }

    pub fn create(&self, input: CreateProfileInput) -> AppResult<Profile> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ErrorCode::ProfileNameRequired.into());
        }
        veil_core::validate(
            &input.fingerprint,
            ProfileOs::from_str_or_default(&input.os),
        )?;
        self.validate_location_against_proxy(&input.fingerprint, input.proxy_id.as_deref())?;
        let browser_version = normalize_browser_version(&input.browser_version);
        let create = CreateProfile {
            name: name.clone(),
            avatar_color: avatar_color(&name),
            group_id: input.group_id,
            tag_ids: input.tag_ids,
            proxy_id: input.proxy_id,
            os: input.os,
            platform: input.platform,
            browser_version,
            notes: input.notes,
            startup_page: normalize_startup_page(&input.startup_page),
            startup_url: normalize_startup_url(&input.startup_url),
            fingerprint: input.fingerprint,
        };
        let record = self.db().create_profile(create).map_err(map_write_error)?;
        Ok(Profile::from(record))
    }

    pub fn update(&self, id: &str, input: CreateProfileInput) -> AppResult<Profile> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ErrorCode::ProfileNameRequired.into());
        }
        veil_core::validate(
            &input.fingerprint,
            ProfileOs::from_str_or_default(&input.os),
        )?;
        self.validate_location_against_proxy(&input.fingerprint, input.proxy_id.as_deref())?;
        let update = UpdateProfile {
            name,
            group_id: input.group_id,
            tag_ids: input.tag_ids,
            proxy_id: input.proxy_id,
            os: input.os,
            platform: input.platform,
            browser_version: normalize_browser_version(&input.browser_version),
            notes: input.notes,
            startup_page: normalize_startup_page(&input.startup_page),
            startup_url: normalize_startup_url(&input.startup_url),
            fingerprint: input.fingerprint,
        };
        let record = self
            .db()
            .update_profile(id, update)
            .map_err(map_write_error)?;
        Ok(Profile::from(record))
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        self.db().delete_profile(id).map_err(map_write_error)
    }

    pub fn update_fingerprint(&self, id: &str, fingerprint: Fingerprint) -> AppResult<Profile> {
        // The OS is read off the stored profile before the fingerprint is validated against it: since
        // T06 three of `validate`'s rules are about the machine the profile claims, and the claim
        // lives on `Profile::os` (5.5b). Fetching first is what gives this call site the claim.
        let existing = self.get(id)?;
        veil_core::validate(&fingerprint, ProfileOs::from_str_or_default(&existing.os))?;
        self.update(
            id,
            CreateProfileInput {
                name: existing.name,
                group_id: existing.group_id,
                tag_ids: existing.tag_ids,
                proxy_id: existing.proxy_id,
                os: existing.os,
                platform: existing.platform,
                browser_version: existing.browser_version,
                notes: existing.notes,
                startup_page: existing.startup_page,
                startup_url: existing.startup_url,
                fingerprint,
            },
        )
    }

    pub fn set_status(&self, id: &str, status: &str, mark_launched: bool) -> AppResult<Profile> {
        let db = self.db();
        db.set_profile_status(id, status, mark_launched)
            .map_err(map_write_error)?;
        let record = db.get_profile(id).map_err(map_read_error)?;
        Ok(Profile::from(record))
    }

    pub fn reset_untracked_active_statuses(
        &self,
        tracked_profile_ids: &BTreeSet<String>,
    ) -> AppResult<usize> {
        let db = self.db();
        let records = db.list_profiles().map_err(map_read_error)?;
        let mut reset = 0usize;
        for record in records {
            if is_active_runtime_status(&record.status) && !tracked_profile_ids.contains(&record.id)
            {
                db.set_profile_status(&record.id, "idle", false)
                    .map_err(map_write_error)?;
                reset += 1;
            }
        }
        Ok(reset)
    }

    pub fn list_groups(&self) -> AppResult<Vec<Group>> {
        let records = self.db().list_groups().map_err(map_read_error)?;
        Ok(records.into_iter().map(Group::from).collect())
    }

    pub fn create_group(&self, input: CreateGroupInput) -> AppResult<Group> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ErrorCode::ProfileNameRequired.into());
        }
        let create = CreateGroup {
            name,
            color: input.color,
        };
        let record = self.db().create_group(create).map_err(map_write_error)?;
        Ok(Group::from(record))
    }

    pub fn update_group(&self, id: &str, input: UpdateGroupInput) -> AppResult<Group> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ErrorCode::ProfileNameRequired.into());
        }
        let record = self
            .db()
            .update_group(
                id,
                UpdateGroup {
                    name,
                    color: input.color,
                },
            )
            .map_err(map_write_error)?;
        Ok(Group::from(record))
    }

    pub fn delete_group(&self, id: &str) -> AppResult<()> {
        self.db().delete_group(id).map_err(map_write_error)
    }

    pub fn get_cookies(&self, profile_id: &str) -> AppResult<ProfileCookieJar> {
        let record = self
            .db()
            .get_profile_cookie_jar(profile_id)
            .map_err(map_read_error)?;
        let cookies = serde_json::from_str(&record.cookies_json)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseReadFailed))?;
        Ok(ProfileCookieJar {
            profile_id: record.profile_id,
            cookies,
            updated_at: record.updated_at,
        })
    }

    pub fn update_cookies(
        &self,
        profile_id: &str,
        input: UpdateProfileCookiesInput,
    ) -> AppResult<ProfileCookieJar> {
        let cookies_json = serde_json::to_string(&input.cookies)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        let record = self
            .db()
            .upsert_profile_cookie_jar(profile_id, cookies_json)
            .map_err(map_write_error)?;
        let cookies = serde_json::from_str(&record.cookies_json)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseReadFailed))?;
        Ok(ProfileCookieJar {
            profile_id: record.profile_id,
            cookies,
            updated_at: record.updated_at,
        })
    }

    pub fn list_credentials(&self, profile_id: &str) -> AppResult<Vec<ProfileCredential>> {
        let records = self
            .db()
            .list_profile_credentials(profile_id)
            .map_err(map_read_error)?;
        Ok(records.into_iter().map(ProfileCredential::from).collect())
    }

    pub fn create_credential(
        &self,
        profile_id: &str,
        input: UpsertProfileCredentialInput,
    ) -> AppResult<ProfileCredential> {
        validate_credential(&input)?;
        let record = self
            .db()
            .create_profile_credential(CreateProfileCredential {
                profile_id: profile_id.to_string(),
                platform: input.platform,
                username: input.username,
                password: input.password,
                notes: input.notes,
            })
            .map_err(map_write_error)?;
        Ok(ProfileCredential::from(record))
    }

    pub fn update_credential(
        &self,
        credential_id: &str,
        input: UpsertProfileCredentialInput,
    ) -> AppResult<ProfileCredential> {
        validate_credential(&input)?;
        let record = self
            .db()
            .update_profile_credential(
                credential_id,
                UpdateProfileCredential {
                    platform: input.platform,
                    username: input.username,
                    password: input.password,
                    notes: input.notes,
                },
            )
            .map_err(map_write_error)?;
        Ok(ProfileCredential::from(record))
    }

    pub fn delete_credential(&self, credential_id: &str) -> AppResult<()> {
        self.db()
            .delete_profile_credential(credential_id)
            .map_err(map_write_error)
    }

    /// Generate a coherent random fingerprint for `os`. Deterministic given `seed` so it is
    /// testable; randomization lives in the backend, not the View.
    pub fn randomize_fingerprint(&self, os: &str, seed: u64) -> Fingerprint {
        veil_runtime::randomize(os, seed)
    }

    /// Reject an inconsistent fingerprint with a stable error code.
    pub fn validate_fingerprint(&self, fp: &Fingerprint, os: ProfileOs) -> AppResult<()> {
        veil_core::validate(fp, os).map_err(AppError::from)
    }

    /// T22's proxy-exit half of the geography rule, applied where the proxy is known.
    ///
    /// `veil_core::validate` deliberately takes no proxy: it checks the stored declaration so a
    /// profile stays valid across an engine swap, and its coordinates rules (well-formed WGS84
    /// `"lat,lng"`, and a timezone to locate them in time) already ran by the time this is called.
    /// The one fact the stored declaration cannot carry is what the proxy's exit reported — and the
    /// proxy test is the only ground truth Veil has, because Veil is offline and holds no geo-IP
    /// table. So: a profile that declares coordinates beside a proxy whose last test reported a
    /// timezone must declare *that* timezone. A proxy with no tested timezone has no ground truth
    /// and stays silent, as does a missing or deleted proxy id (the launch path's own convention).
    fn validate_location_against_proxy(
        &self,
        fingerprint: &Fingerprint,
        proxy_id: Option<&str>,
    ) -> AppResult<()> {
        if fingerprint.location.trim().is_empty() {
            return Ok(());
        }
        let Some(proxy_id) = proxy_id.map(str::trim).filter(|id| !id.is_empty()) else {
            return Ok(());
        };
        let record = match self.db().get_proxy(proxy_id) {
            Ok(record) => record,
            // A stale proxy id must not block the save; there is nothing to agree with.
            Err(_) => return Ok(()),
        };
        let Some(proxy_timezone) = record
            .timezone
            .as_deref()
            .map(str::trim)
            .filter(|tz| !tz.is_empty())
        else {
            return Ok(());
        };
        if fingerprint.timezone.trim() != proxy_timezone {
            return Err(ErrorCode::FingerprintInvalid.into());
        }
        Ok(())
    }
}

fn default_startup_page() -> String {
    "veil".to_string()
}

fn normalize_browser_version(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_startup_page(value: &str) -> String {
    match value {
        "google" | "bing" | "custom" => value.to_string(),
        _ => "veil".to_string(),
    }
}

fn normalize_startup_url(value: &str) -> String {
    value.trim().to_string()
}

fn map_read_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::ProfileNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseReadFailed.into()
        }
    }
}

fn map_write_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::ProfileNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseWriteFailed.into()
        }
    }
}

const AVATAR_COLORS: [&str; 6] = [
    "#5640c9", "#18935a", "#c2790b", "#2d6fd6", "#b02a20", "#7e6ddd",
];

fn avatar_color(seed: &str) -> String {
    let sum: usize = seed.bytes().map(|byte| byte as usize).sum();
    AVATAR_COLORS[sum % AVATAR_COLORS.len()].to_string()
}

fn is_active_runtime_status(status: &str) -> bool {
    matches!(status, "launching" | "running")
}

fn validate_credential(input: &UpsertProfileCredentialInput) -> AppResult<()> {
    if input.platform.trim().is_empty() || input.username.trim().is_empty() {
        return Err(ErrorCode::ProfileNameRequired.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ProfileService {
        let db = Database::open_in_memory("test-key").unwrap();
        ProfileService::new(Arc::new(Mutex::new(db)))
    }

    fn input(name: &str) -> CreateProfileInput {
        CreateProfileInput {
            name: name.to_string(),
            group_id: None,
            tag_ids: vec![],
            proxy_id: None,
            os: "windows".to_string(),
            platform: "Storefront".to_string(),
            browser_version: String::new(),
            notes: String::new(),
            startup_page: "veil".to_string(),
            startup_url: String::new(),
            fingerprint: veil_core::Fingerprint::default(),
        }
    }

    #[test]
    fn creates_lists_and_deletes_a_profile() {
        let svc = service();
        let profile = svc.create(input("Alpha")).unwrap();
        assert_eq!(profile.name, "Alpha");
        assert_eq!(profile.status, "idle");
        assert_eq!(profile.browser_version, "");

        let all = svc.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, profile.id);

        svc.delete(&profile.id).unwrap();
        assert!(svc.list().unwrap().is_empty());
    }

    #[test]
    fn rejects_empty_name() {
        let svc = service();
        let err = svc.create(input("   ")).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ProfileNameRequired);
    }

    #[test]
    fn updates_and_sets_status() {
        let svc = service();
        let profile = svc.create(input("Bravo")).unwrap();
        let updated = svc
            .update(
                &profile.id,
                CreateProfileInput {
                    name: "Bravo Edited".to_string(),
                    group_id: None,
                    tag_ids: vec![],
                    proxy_id: None,
                    os: "macos".to_string(),
                    platform: "Ads".to_string(),
                    browser_version: "121".to_string(),
                    notes: "edited".to_string(),
                    startup_page: "google".to_string(),
                    startup_url: String::new(),
                    fingerprint: veil_core::Fingerprint::default(),
                },
            )
            .unwrap();
        assert_eq!(updated.name, "Bravo Edited");
        assert_eq!(updated.os, "macos");
        assert_eq!(updated.browser_version, "121");
        assert_eq!(updated.startup_page, "google");

        let running = svc.set_status(&profile.id, "running", true).unwrap();
        assert_eq!(running.status, "running");
        assert!(running.last_launched_at.is_some());

        let idle = svc.set_status(&profile.id, "idle", false).unwrap();
        assert_eq!(idle.status, "idle");
        // last_launched_at must be preserved when mark_launched is false.
        assert!(idle.last_launched_at.is_some());
    }

    #[test]
    fn resets_active_statuses_that_are_not_tracked() {
        let svc = service();
        let tracked = svc.create(input("Tracked")).unwrap();
        let stale = svc.create(input("Stale")).unwrap();
        let idle = svc.create(input("Idle")).unwrap();

        svc.set_status(&tracked.id, "running", true).unwrap();
        svc.set_status(&stale.id, "launching", false).unwrap();

        let tracked_ids = BTreeSet::from([tracked.id.clone()]);
        let reset = svc.reset_untracked_active_statuses(&tracked_ids).unwrap();

        assert_eq!(reset, 1);
        assert_eq!(svc.get(&tracked.id).unwrap().status, "running");
        assert_eq!(svc.get(&stale.id).unwrap().status, "idle");
        assert_eq!(svc.get(&idle.id).unwrap().status, "idle");
    }

    #[test]
    fn group_crud_round_trip() {
        let svc = service();
        let group = svc
            .create_group(CreateGroupInput {
                name: "Team".to_string(),
                color: Some("#5640c9".into()),
            })
            .unwrap();
        assert_eq!(group.name, "Team");
        let groups = svc.list_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, group.id);
    }

    #[test]
    fn randomize_is_deterministic_and_persists() {
        let svc = service();
        let fp = svc.randomize_fingerprint("macos", 99);
        assert_eq!(fp, svc.randomize_fingerprint("macos", 99));
        assert!(svc.validate_fingerprint(&fp, ProfileOs::Macos).is_ok());

        let mut inp = input("FP");
        // The profile has to claim the OS the preset was drawn for. A macOS preset declares a Retina
        // panel and sizes its window against the macOS menu-bar inset; stored on a Windows profile it
        // is a window 23 px taller than the work area a Windows taskbar leaves, which `validate`
        // rejects — correctly, and it is the cross-OS edit T06 added the rule for.
        inp.os = "macos".to_string();
        inp.fingerprint = fp.clone();
        let saved = svc.create(inp).unwrap();
        assert_eq!(saved.fingerprint, fp);
    }

    #[test]
    fn rejects_inconsistent_fingerprint() {
        let svc = service();
        let mut inp = input("Bad");
        inp.fingerprint = veil_core::Fingerprint {
            hardware_concurrency: 200,
            ..Default::default()
        };
        assert_eq!(
            svc.create(inp).unwrap_err().code(),
            ErrorCode::FingerprintInvalid
        );
    }

    #[test]
    fn missing_profile_maps_to_profile_not_found() {
        let svc = service();
        let err = svc.get("nope").unwrap_err();
        assert_eq!(err.code(), ErrorCode::ProfileNotFound);
        let err = svc.delete("nope").unwrap_err();
        assert_eq!(err.code(), ErrorCode::ProfileNotFound);
    }
}
