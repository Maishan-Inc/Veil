//! Encrypted local persistence for Veil (SQLCipher via `rusqlite`).
//!
//! This crate is the only SQL boundary. The database is opened with a key (derived from a
//! master password or sealed via DPAPI in `veil-secure-storage`) applied with the SQLCipher
//! `key` pragma before any other access.

mod ai;
mod extensions;
mod groups;
mod local_api;
mod profile_data;
mod profiles;
mod proxies;
mod schema;

use std::path::Path;

use rusqlite::{params, params_from_iter, Connection, Row};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

pub use ai::{
    AiAuditLogFilter, AiAuditLogRecord, AiModelConfigRecord, AiProfileFactRecord, AiSettingsRecord,
    AiTaskEventRecord, AiTaskRecord, CreateAiTask, CreateAiTaskEvent, UpdateAiTask,
    UpsertAiModelConfig, UpsertAiProfileFact, UpsertAiSettings,
};
pub use extensions::{BrowserExtensionRecord, CreateBrowserExtension, UpdateBrowserExtension};
pub use groups::{CreateGroup, GroupRecord, UpdateGroup};
pub use local_api::{
    CreateLocalApiKey, LocalApiKeyRecord, LocalApiSettingsRecord, UpsertLocalApiSettings,
};
pub use profile_data::{
    CreateProfileCredential, ProfileCookieJarRecord, ProfileCredentialRecord,
    UpdateProfileCredential,
};
pub use profiles::{CreateProfile, ProfileRecord, UpdateProfile};
pub use proxies::{CreateProxy, ProxyLocation, ProxyRecord, UpdateProxy};

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("record not found")]
    NotFound,
}

const PROFILE_COLUMNS: &str = "id, name, avatar_color, group_id, tag_ids, proxy_id, os, platform, \
     browser_version, notes, status, startup_page, startup_url, fingerprint, created_at, \
     updated_at, last_launched_at";

const PROXY_COLUMNS: &str = "id, name, kind, host, port, username, password, country, \
     detected_ip, country_code, region, city, timezone, created_at, updated_at";
const AI_SETTINGS_COLUMNS: &str = "id, provider_kind, base_url, model, reasoning_effort, \
     review_mode, network_enabled, api_key, updated_at";
const AI_MODEL_CONFIG_COLUMNS: &str = "id, name, provider_kind, icon, base_url, model, \
     reasoning_effort, review_mode, network_enabled, api_key, is_default, created_at, updated_at";
const AI_TASK_COLUMNS: &str = "id, profile_id, model_config_id, prompt, execution_mode, status, \
     session_id, cdp_url, requires_user_action, pending_approval_code, pending_approval_message, \
     human_verification_policy, payment_policy, created_at, updated_at";
const AI_TASK_EVENT_COLUMNS: &str = "id, task_id, actor, kind, action, message, created_at";
const AI_PROFILE_FACT_COLUMNS: &str = "id, profile_id, key_name, value_text, source, \
     sensitivity, created_at, updated_at";
const BROWSER_EXTENSION_COLUMNS: &str = "id, name, source, store_url, store_id, version, \
     install_path, default_enabled, created_at, updated_at";
const LOCAL_API_SETTINGS_COLUMNS: &str = "id, enabled, port, bind_mode, updated_at";
const LOCAL_API_KEY_COLUMNS: &str =
    "id, name, key_secret, suffix, created_at, updated_at, last_used_at";
const PROFILE_CREDENTIAL_COLUMNS: &str =
    "id, profile_id, platform, username, password, notes, created_at, updated_at";

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path, key: &str) -> Result<Self, DatabaseError> {
        Self::init(Connection::open(path)?, key)
    }

    pub fn open_in_memory(key: &str) -> Result<Self, DatabaseError> {
        Self::init(Connection::open_in_memory()?, key)
    }

    fn init(conn: Connection, key: &str) -> Result<Self, DatabaseError> {
        // SQLCipher: the key must be set before touching any table.
        conn.pragma_update(None, "key", key)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    fn now() -> String {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default()
    }

    // ---- Profiles ------------------------------------------------------------

    pub fn create_profile(&self, input: CreateProfile) -> Result<ProfileRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        let tag_ids = serde_json::to_string(&input.tag_ids)?;
        let fingerprint = serde_json::to_string(&input.fingerprint)?;
        self.conn.execute(
            "INSERT INTO profiles (id, name, avatar_color, group_id, tag_ids, proxy_id, os, \
             platform, browser_version, notes, status, startup_page, startup_url, fingerprint, \
             created_at, updated_at, last_launched_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'idle', ?11, ?12, ?13, ?14, ?14, NULL)",
            params![
                id,
                input.name.trim(),
                input.avatar_color,
                input.group_id,
                tag_ids,
                input.proxy_id,
                input.os,
                input.platform,
                input.browser_version,
                input.notes,
                input.startup_page,
                input.startup_url,
                fingerprint,
                now,
            ],
        )?;
        self.get_profile(&id)
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileRecord>, DatabaseError> {
        let sql = format!("SELECT {PROFILE_COLUMNS} FROM profiles ORDER BY updated_at DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_profile)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_profile(&self, id: &str) -> Result<ProfileRecord, DatabaseError> {
        let sql = format!("SELECT {PROFILE_COLUMNS} FROM profiles WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_profile)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn update_profile(
        &self,
        id: &str,
        input: UpdateProfile,
    ) -> Result<ProfileRecord, DatabaseError> {
        let now = Self::now();
        let tag_ids = serde_json::to_string(&input.tag_ids)?;
        let fingerprint = serde_json::to_string(&input.fingerprint)?;
        let changed = self.conn.execute(
            "UPDATE profiles SET name = ?2, group_id = ?3, tag_ids = ?4, proxy_id = ?5, os = ?6, \
             platform = ?7, browser_version = ?8, notes = ?9, startup_page = ?10, \
             startup_url = ?11, fingerprint = ?12, updated_at = ?13 \
             WHERE id = ?1",
            params![
                id,
                input.name.trim(),
                input.group_id,
                tag_ids,
                input.proxy_id,
                input.os,
                input.platform,
                input.browser_version,
                input.notes,
                input.startup_page,
                input.startup_url,
                fingerprint,
                now,
            ],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_profile(id)
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), DatabaseError> {
        let changed = self
            .conn
            .execute("DELETE FROM profiles WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    /// Update a profile's launch status, optionally stamping `last_launched_at`.
    pub fn set_profile_status(
        &self,
        id: &str,
        status: &str,
        mark_launched: bool,
    ) -> Result<(), DatabaseError> {
        let now = Self::now();
        let changed = if mark_launched {
            self.conn.execute(
                "UPDATE profiles SET status = ?2, last_launched_at = ?3, updated_at = ?3 \
                 WHERE id = ?1",
                params![id, status, now],
            )?
        } else {
            self.conn.execute(
                "UPDATE profiles SET status = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, status, now],
            )?
        };
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    /// Change ONLY the proxy assignment of a profile. Used by batch-modify-proxy so the
    /// caller doesn't need to round-trip every other field.
    pub fn set_profile_proxy(&self, id: &str, proxy_id: Option<&str>) -> Result<(), DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE profiles SET proxy_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, proxy_id, now],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    // ---- Browser extensions -------------------------------------------------

    pub fn create_browser_extension(
        &self,
        input: CreateBrowserExtension,
    ) -> Result<BrowserExtensionRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO browser_extensions (id, name, source, store_url, store_id, version, \
             install_path, default_enabled, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id,
                input.name.trim(),
                input.source,
                input.store_url,
                input.store_id,
                input.version,
                input.install_path,
                input.default_enabled,
                now,
            ],
        )?;
        self.get_browser_extension(&id)
    }

    pub fn list_browser_extensions(&self) -> Result<Vec<BrowserExtensionRecord>, DatabaseError> {
        let sql = format!(
            "SELECT {BROWSER_EXTENSION_COLUMNS} FROM browser_extensions ORDER BY updated_at DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_browser_extension)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_browser_extension(&self, id: &str) -> Result<BrowserExtensionRecord, DatabaseError> {
        let sql =
            format!("SELECT {BROWSER_EXTENSION_COLUMNS} FROM browser_extensions WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_browser_extension)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn update_browser_extension(
        &self,
        id: &str,
        input: UpdateBrowserExtension,
    ) -> Result<BrowserExtensionRecord, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE browser_extensions SET name = ?2, default_enabled = ?3, updated_at = ?4 \
             WHERE id = ?1",
            params![id, input.name.trim(), input.default_enabled, now],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_browser_extension(id)
    }

    pub fn delete_browser_extension(&self, id: &str) -> Result<(), DatabaseError> {
        let changed = self
            .conn
            .execute("DELETE FROM browser_extensions WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    pub fn list_extension_profile_ids(
        &self,
        extension_id: &str,
    ) -> Result<Vec<String>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT profile_id FROM profile_extensions WHERE extension_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([extension_id], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_profile_extensions(
        &self,
        profile_id: &str,
    ) -> Result<Vec<BrowserExtensionRecord>, DatabaseError> {
        let sql = format!(
            "SELECT {BROWSER_EXTENSION_COLUMNS} FROM browser_extensions \
             INNER JOIN profile_extensions ON browser_extensions.id = profile_extensions.extension_id \
             WHERE profile_extensions.profile_id = ?1 ORDER BY browser_extensions.name ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([profile_id], map_browser_extension)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn replace_extension_profiles(
        &self,
        extension_id: &str,
        profile_ids: &[String],
    ) -> Result<(), DatabaseError> {
        self.get_browser_extension(extension_id)?;
        for profile_id in profile_ids {
            self.get_profile(profile_id)?;
        }
        let now = Self::now();
        self.conn.execute(
            "DELETE FROM profile_extensions WHERE extension_id = ?1",
            params![extension_id],
        )?;
        for profile_id in profile_ids {
            self.conn.execute(
                "INSERT OR IGNORE INTO profile_extensions (profile_id, extension_id, created_at) \
                 VALUES (?1, ?2, ?3)",
                params![profile_id, extension_id, now],
            )?;
        }
        Ok(())
    }

    pub fn assign_default_extensions_to_profile(
        &self,
        profile_id: &str,
    ) -> Result<(), DatabaseError> {
        self.get_profile(profile_id)?;
        let now = Self::now();
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM browser_extensions WHERE default_enabled = 1")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for extension_id in rows.collect::<rusqlite::Result<Vec<_>>>()? {
            self.conn.execute(
                "INSERT OR IGNORE INTO profile_extensions (profile_id, extension_id, created_at) \
                 VALUES (?1, ?2, ?3)",
                params![profile_id, extension_id, now],
            )?;
        }
        Ok(())
    }

    // ---- Proxies -------------------------------------------------------------

    pub fn create_proxy(&self, input: CreateProxy) -> Result<ProxyRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO proxies (id, name, kind, host, port, username, password, country, \
             created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id,
                input.name.trim(),
                input.kind,
                input.host.trim(),
                input.port,
                input.username,
                input.password,
                input.country,
                now,
            ],
        )?;
        self.get_proxy(&id)
    }

    pub fn list_proxies(&self) -> Result<Vec<ProxyRecord>, DatabaseError> {
        let sql = format!("SELECT {PROXY_COLUMNS} FROM proxies ORDER BY name ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_proxy)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_proxy(&self, id: &str) -> Result<ProxyRecord, DatabaseError> {
        let sql = format!("SELECT {PROXY_COLUMNS} FROM proxies WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_proxy)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    /// Update a proxy. When `input.password` is `None` the existing password is
    /// preserved; pass `Some(String::new())` to clear it explicitly.
    pub fn update_proxy(&self, id: &str, input: UpdateProxy) -> Result<ProxyRecord, DatabaseError> {
        let now = Self::now();
        // Two distinct UPDATE paths keeps the SQL simple and side-steps having to express
        // "leave the column alone" inside a CASE expression with rusqlite param binding.
        let changed = if let Some(password) = input.password.as_deref() {
            self.conn.execute(
                "UPDATE proxies SET name = ?2, kind = ?3, host = ?4, port = ?5, username = ?6, \
                 password = ?7, country = ?8, updated_at = ?9 WHERE id = ?1",
                params![
                    id,
                    input.name.trim(),
                    input.kind,
                    input.host.trim(),
                    input.port,
                    input.username,
                    password,
                    input.country,
                    now,
                ],
            )?
        } else {
            self.conn.execute(
                "UPDATE proxies SET name = ?2, kind = ?3, host = ?4, port = ?5, username = ?6, \
                 country = ?7, updated_at = ?8 WHERE id = ?1",
                params![
                    id,
                    input.name.trim(),
                    input.kind,
                    input.host.trim(),
                    input.port,
                    input.username,
                    input.country,
                    now,
                ],
            )?
        };
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_proxy(id)
    }

    /// Persist the latest user-initiated proxy probe metadata. This never changes the
    /// proxy endpoint or credentials; it only records what the proxy resolved to.
    pub fn update_proxy_location(
        &self,
        id: &str,
        location: ProxyLocation,
    ) -> Result<ProxyRecord, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE proxies SET country = COALESCE(?2, country), detected_ip = ?3, \
             country_code = ?4, region = ?5, city = ?6, timezone = ?7, updated_at = ?8 \
             WHERE id = ?1",
            params![
                id,
                location.country,
                location.detected_ip,
                location.country_code,
                location.region,
                location.city,
                location.timezone,
                now,
            ],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_proxy(id)
    }

    pub fn delete_proxy(&self, id: &str) -> Result<(), DatabaseError> {
        let changed = self
            .conn
            .execute("DELETE FROM proxies WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    // ---- Groups --------------------------------------------------------------

    pub fn create_group(&self, input: CreateGroup) -> Result<GroupRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO groups (id, name, color, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, input.name.trim(), input.color, now],
        )?;
        self.get_group(&id)
    }

    pub fn get_group(&self, id: &str) -> Result<GroupRecord, DatabaseError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color, created_at, updated_at FROM groups WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], map_group)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn list_groups(&self) -> Result<Vec<GroupRecord>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, created_at, updated_at FROM groups ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], map_group)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn delete_group(&self, id: &str) -> Result<(), DatabaseError> {
        let changed = self
            .conn
            .execute("DELETE FROM groups WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    pub fn update_group(&self, id: &str, input: UpdateGroup) -> Result<GroupRecord, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE groups SET name = ?2, color = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, input.name.trim(), input.color, now],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_group(id)
    }

    // ---- Profile cookies / credentials --------------------------------------

    pub fn get_profile_cookie_jar(
        &self,
        profile_id: &str,
    ) -> Result<ProfileCookieJarRecord, DatabaseError> {
        self.get_profile(profile_id)?;
        let mut stmt = self.conn.prepare(
            "SELECT profile_id, cookies_json, updated_at FROM profile_cookie_jars WHERE profile_id = ?1",
        )?;
        let mut rows = stmt.query_map([profile_id], map_profile_cookie_jar)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Ok(ProfileCookieJarRecord {
                profile_id: profile_id.to_string(),
                cookies_json: "[]".to_string(),
                updated_at: Self::now(),
            }),
        }
    }

    pub fn upsert_profile_cookie_jar(
        &self,
        profile_id: &str,
        cookies_json: String,
    ) -> Result<ProfileCookieJarRecord, DatabaseError> {
        self.get_profile(profile_id)?;
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO profile_cookie_jars (profile_id, cookies_json, updated_at) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(profile_id) DO UPDATE SET cookies_json = excluded.cookies_json, \
             updated_at = excluded.updated_at",
            params![profile_id, cookies_json, now],
        )?;
        self.get_profile_cookie_jar(profile_id)
    }

    pub fn list_profile_credentials(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ProfileCredentialRecord>, DatabaseError> {
        self.get_profile(profile_id)?;
        let sql = format!(
            "SELECT {PROFILE_CREDENTIAL_COLUMNS} FROM profile_credentials \
             WHERE profile_id = ?1 ORDER BY updated_at DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([profile_id], map_profile_credential)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_profile_credential(
        &self,
        input: CreateProfileCredential,
    ) -> Result<ProfileCredentialRecord, DatabaseError> {
        self.get_profile(&input.profile_id)?;
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO profile_credentials (id, profile_id, platform, username, password, notes, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                id,
                input.profile_id,
                input.platform.trim(),
                input.username.trim(),
                input.password,
                input.notes,
                now,
            ],
        )?;
        self.get_profile_credential(&id)
    }

    pub fn get_profile_credential(
        &self,
        credential_id: &str,
    ) -> Result<ProfileCredentialRecord, DatabaseError> {
        let sql =
            format!("SELECT {PROFILE_CREDENTIAL_COLUMNS} FROM profile_credentials WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([credential_id], map_profile_credential)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn update_profile_credential(
        &self,
        credential_id: &str,
        input: UpdateProfileCredential,
    ) -> Result<ProfileCredentialRecord, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE profile_credentials SET platform = ?2, username = ?3, password = ?4, \
             notes = ?5, updated_at = ?6 WHERE id = ?1",
            params![
                credential_id,
                input.platform.trim(),
                input.username.trim(),
                input.password,
                input.notes,
                now,
            ],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_profile_credential(credential_id)
    }

    pub fn delete_profile_credential(&self, credential_id: &str) -> Result<(), DatabaseError> {
        let changed = self.conn.execute(
            "DELETE FROM profile_credentials WHERE id = ?1",
            params![credential_id],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        Ok(())
    }

    // ---- Local API -----------------------------------------------------------

    pub fn get_local_api_settings(&self) -> Result<LocalApiSettingsRecord, DatabaseError> {
        let sql = format!(
            "SELECT {LOCAL_API_SETTINGS_COLUMNS} FROM local_api_settings WHERE id = 'default'"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([], map_local_api_settings)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn upsert_local_api_settings(
        &self,
        input: UpsertLocalApiSettings,
    ) -> Result<LocalApiSettingsRecord, DatabaseError> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO local_api_settings (id, enabled, port, bind_mode, updated_at) \
             VALUES ('default', ?1, ?2, ?3, ?4) \
             ON CONFLICT(id) DO UPDATE SET enabled = excluded.enabled, port = excluded.port, \
             bind_mode = excluded.bind_mode, updated_at = excluded.updated_at",
            params![input.enabled, input.port, input.bind_mode, now],
        )?;
        self.get_local_api_settings()
    }

    pub fn create_local_api_key(
        &self,
        input: CreateLocalApiKey,
    ) -> Result<LocalApiKeyRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO local_api_keys (id, name, key_secret, suffix, created_at, updated_at, \
             last_used_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5, NULL)",
            params![id, input.name.trim(), input.key_secret, input.suffix, now],
        )?;
        self.get_local_api_key(&id)
    }

    pub fn list_local_api_keys(&self) -> Result<Vec<LocalApiKeyRecord>, DatabaseError> {
        let sql =
            format!("SELECT {LOCAL_API_KEY_COLUMNS} FROM local_api_keys ORDER BY updated_at DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_local_api_key)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_local_api_key(&self, id: &str) -> Result<LocalApiKeyRecord, DatabaseError> {
        let sql = format!("SELECT {LOCAL_API_KEY_COLUMNS} FROM local_api_keys WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_local_api_key)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn rotate_local_api_key(
        &self,
        id: &str,
        key_secret: String,
        suffix: String,
    ) -> Result<LocalApiKeyRecord, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE local_api_keys SET key_secret = ?2, suffix = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, key_secret, suffix, now],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_local_api_key(id)
    }

    /// Resolve a key by its full secret — the HTTP listener's authentication path. The
    /// secrets are high-entropy (`veil_<uuid><uuid>`), so an exact-match lookup is the
    /// whole check; the row is returned so callers can also touch `last_used_at`.
    pub fn find_local_api_key_by_secret(
        &self,
        key_secret: &str,
    ) -> Result<LocalApiKeyRecord, DatabaseError> {
        let sql =
            format!("SELECT {LOCAL_API_KEY_COLUMNS} FROM local_api_keys WHERE key_secret = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([key_secret], map_local_api_key)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    /// Record that a key was used, so the API console can show a real `last_used_at`.
    pub fn touch_local_api_key_used(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE local_api_keys SET last_used_at = ?2 WHERE id = ?1",
            params![id, Self::now()],
        )?;
        Ok(())
    }

    // ---- AI settings --------------------------------------------------------

    pub fn get_ai_settings(&self) -> Result<AiSettingsRecord, DatabaseError> {
        let sql = format!("SELECT {AI_SETTINGS_COLUMNS} FROM ai_settings WHERE id = 'default'");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([], map_ai_settings)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn upsert_ai_settings(
        &self,
        input: UpsertAiSettings,
    ) -> Result<AiSettingsRecord, DatabaseError> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO ai_settings (id, provider_kind, base_url, model, reasoning_effort, \
             review_mode, network_enabled, api_key, updated_at) \
             VALUES ('default', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET provider_kind = excluded.provider_kind, \
             base_url = excluded.base_url, model = excluded.model, \
             reasoning_effort = excluded.reasoning_effort, review_mode = excluded.review_mode, \
             network_enabled = excluded.network_enabled, \
             api_key = excluded.api_key, updated_at = excluded.updated_at",
            params![
                input.provider_kind,
                input.base_url,
                input.model,
                input.reasoning_effort,
                input.review_mode,
                input.network_enabled,
                input.api_key,
                now,
            ],
        )?;
        self.get_ai_settings()
    }

    pub fn list_ai_model_configs(&self) -> Result<Vec<AiModelConfigRecord>, DatabaseError> {
        let sql = format!(
            "SELECT {AI_MODEL_CONFIG_COLUMNS} FROM ai_model_configs \
             ORDER BY is_default DESC, updated_at DESC, name ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_ai_model_config)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_ai_model_config(&self, id: &str) -> Result<AiModelConfigRecord, DatabaseError> {
        let sql = format!("SELECT {AI_MODEL_CONFIG_COLUMNS} FROM ai_model_configs WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_ai_model_config)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn get_default_ai_model_config(&self) -> Result<AiModelConfigRecord, DatabaseError> {
        let sql = format!(
            "SELECT {AI_MODEL_CONFIG_COLUMNS} FROM ai_model_configs \
             ORDER BY is_default DESC, updated_at DESC LIMIT 1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([], map_ai_model_config)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn upsert_ai_model_config(
        &self,
        input: UpsertAiModelConfig,
    ) -> Result<AiModelConfigRecord, DatabaseError> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Self::now();
        if input.is_default {
            self.conn
                .execute("UPDATE ai_model_configs SET is_default = 0", [])?;
        }
        self.conn.execute(
            "INSERT INTO ai_model_configs (id, name, provider_kind, icon, base_url, model, \
             reasoning_effort, review_mode, network_enabled, api_key, is_default, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, provider_kind = excluded.provider_kind, \
             icon = excluded.icon, base_url = excluded.base_url, model = excluded.model, \
             reasoning_effort = excluded.reasoning_effort, review_mode = excluded.review_mode, \
             network_enabled = excluded.network_enabled, api_key = excluded.api_key, \
             is_default = excluded.is_default, updated_at = excluded.updated_at",
            params![
                id,
                input.name,
                input.provider_kind,
                input.icon,
                input.base_url,
                input.model,
                input.reasoning_effort,
                input.review_mode,
                input.network_enabled,
                input.api_key,
                input.is_default,
                now,
            ],
        )?;
        self.get_ai_model_config(&id)
    }

    // ---- AI tasks -----------------------------------------------------------

    pub fn create_ai_task(&self, input: CreateAiTask) -> Result<AiTaskRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO ai_tasks (id, profile_id, model_config_id, prompt, execution_mode, status, \
             session_id, cdp_url, requires_user_action, pending_approval_code, pending_approval_message, \
             human_verification_policy, payment_policy, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
            params![
                id,
                input.profile_id,
                input.model_config_id,
                input.prompt,
                input.execution_mode,
                input.status,
                input.session_id,
                input.cdp_url,
                input.requires_user_action,
                input.pending_approval_code,
                input.pending_approval_message,
                input.human_verification_policy,
                input.payment_policy,
                now,
            ],
        )?;
        self.get_ai_task(&id)
    }

    pub fn get_ai_task(&self, id: &str) -> Result<AiTaskRecord, DatabaseError> {
        let sql = format!("SELECT {AI_TASK_COLUMNS} FROM ai_tasks WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_ai_task)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn list_ai_tasks_for_profile(
        &self,
        profile_id: &str,
    ) -> Result<Vec<AiTaskRecord>, DatabaseError> {
        let sql = format!(
            "SELECT {AI_TASK_COLUMNS} FROM ai_tasks WHERE profile_id = ?1 ORDER BY updated_at DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([profile_id], map_ai_task)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn update_ai_task(
        &self,
        id: &str,
        input: UpdateAiTask,
    ) -> Result<AiTaskRecord, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE ai_tasks SET status = ?2, session_id = ?3, cdp_url = ?4, \
             requires_user_action = ?5, pending_approval_code = ?6, \
             pending_approval_message = ?7, updated_at = ?8 WHERE id = ?1",
            params![
                id,
                input.status,
                input.session_id,
                input.cdp_url,
                input.requires_user_action,
                input.pending_approval_code,
                input.pending_approval_message,
                now,
            ],
        )?;
        if changed == 0 {
            return Err(DatabaseError::NotFound);
        }
        self.get_ai_task(id)
    }

    pub fn claim_ai_task_for_run(&self, id: &str) -> Result<Option<AiTaskRecord>, DatabaseError> {
        let now = Self::now();
        let changed = self.conn.execute(
            "UPDATE ai_tasks SET status = 'running', requires_user_action = 0, \
             pending_approval_code = NULL, pending_approval_message = NULL, updated_at = ?2 \
             WHERE id = ?1 AND status IN ('ready', 'pausedForUser')",
            params![id, now],
        )?;
        if changed == 1 {
            return self.get_ai_task(id).map(Some);
        }
        match self.get_ai_task(id) {
            Ok(_) => Ok(None),
            Err(DatabaseError::NotFound) => Err(DatabaseError::NotFound),
            Err(err) => Err(err),
        }
    }

    pub fn create_ai_task_event(
        &self,
        input: CreateAiTaskEvent,
    ) -> Result<AiTaskEventRecord, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO ai_task_events (id, task_id, actor, kind, action, message, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                input.task_id,
                input.actor,
                input.kind,
                input.action,
                input.message,
                now
            ],
        )?;
        let sql = format!("SELECT {AI_TASK_EVENT_COLUMNS} FROM ai_task_events WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map([id], map_ai_task_event)?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }

    pub fn list_ai_task_events(
        &self,
        task_id: &str,
    ) -> Result<Vec<AiTaskEventRecord>, DatabaseError> {
        let sql =
            format!("SELECT {AI_TASK_EVENT_COLUMNS} FROM ai_task_events WHERE task_id = ?1 ORDER BY created_at ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([task_id], map_ai_task_event)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_ai_audit_logs(
        &self,
        filter: AiAuditLogFilter,
    ) -> Result<Vec<AiAuditLogRecord>, DatabaseError> {
        let mut sql = String::from(
            "SELECT e.id, e.task_id, t.profile_id, COALESCE(p.name, ''), e.actor, e.kind, \
             e.action, e.message, e.created_at \
             FROM ai_task_events e \
             JOIN ai_tasks t ON t.id = e.task_id \
             LEFT JOIN profiles p ON p.id = t.profile_id \
             WHERE 1 = 1",
        );
        let mut values = Vec::new();
        if let Some(start_at) = filter.start_at {
            sql.push_str(" AND e.created_at >= ?");
            values.push(start_at);
        }
        if let Some(end_at) = filter.end_at {
            sql.push_str(" AND e.created_at <= ?");
            values.push(end_at);
        }
        if let Some(actor) = filter.actor {
            sql.push_str(" AND e.actor = ?");
            values.push(actor);
        }
        sql.push_str(" ORDER BY e.created_at DESC LIMIT 1000");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), map_ai_audit_log)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ---- AI profile facts ---------------------------------------------------

    pub fn list_ai_profile_facts(
        &self,
        profile_id: &str,
    ) -> Result<Vec<AiProfileFactRecord>, DatabaseError> {
        let sql = format!(
            "SELECT {AI_PROFILE_FACT_COLUMNS} FROM ai_profile_facts WHERE profile_id = ?1 ORDER BY updated_at DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([profile_id], map_ai_profile_fact)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn upsert_ai_profile_fact(
        &self,
        input: UpsertAiProfileFact,
    ) -> Result<AiProfileFactRecord, DatabaseError> {
        let now = Self::now();
        self.conn.execute(
            "INSERT INTO ai_profile_facts (id, profile_id, key_name, value_text, source, \
             sensitivity, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7) \
             ON CONFLICT(profile_id, key_name) DO UPDATE SET value_text = excluded.value_text, \
             source = excluded.source, sensitivity = excluded.sensitivity, updated_at = excluded.updated_at",
            params![
                Uuid::new_v4().to_string(),
                input.profile_id,
                input.key_name,
                input.value_text,
                input.source,
                input.sensitivity,
                now,
            ],
        )?;
        let sql = format!(
            "SELECT {AI_PROFILE_FACT_COLUMNS} FROM ai_profile_facts WHERE profile_id = ?1 AND key_name = ?2"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map(
            params![input.profile_id, input.key_name],
            map_ai_profile_fact,
        )?;
        match rows.next() {
            Some(row) => Ok(row?),
            None => Err(DatabaseError::NotFound),
        }
    }
}

fn map_profile(row: &Row<'_>) -> rusqlite::Result<ProfileRecord> {
    let tag_ids: String = row.get(4)?;
    let fingerprint: String = row.get(13)?;
    Ok(ProfileRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        avatar_color: row.get(2)?,
        group_id: row.get(3)?,
        tag_ids: serde_json::from_str(&tag_ids).unwrap_or_default(),
        proxy_id: row.get(5)?,
        os: row.get(6)?,
        platform: row.get(7)?,
        browser_version: row.get(8)?,
        notes: row.get(9)?,
        status: row.get(10)?,
        startup_page: row.get(11)?,
        startup_url: row.get(12)?,
        fingerprint: serde_json::from_str(&fingerprint).unwrap_or_default(),
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        last_launched_at: row.get(16)?,
    })
}

fn map_group(row: &Row<'_>) -> rusqlite::Result<GroupRecord> {
    Ok(GroupRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_local_api_settings(row: &Row<'_>) -> rusqlite::Result<LocalApiSettingsRecord> {
    Ok(LocalApiSettingsRecord {
        id: row.get(0)?,
        enabled: row.get(1)?,
        port: row.get(2)?,
        bind_mode: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_local_api_key(row: &Row<'_>) -> rusqlite::Result<LocalApiKeyRecord> {
    Ok(LocalApiKeyRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        key_secret: row.get(2)?,
        suffix: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        last_used_at: row.get(6)?,
    })
}

fn map_profile_cookie_jar(row: &Row<'_>) -> rusqlite::Result<ProfileCookieJarRecord> {
    Ok(ProfileCookieJarRecord {
        profile_id: row.get(0)?,
        cookies_json: row.get(1)?,
        updated_at: row.get(2)?,
    })
}

fn map_profile_credential(row: &Row<'_>) -> rusqlite::Result<ProfileCredentialRecord> {
    Ok(ProfileCredentialRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        platform: row.get(2)?,
        username: row.get(3)?,
        password: row.get(4)?,
        notes: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn map_proxy(row: &Row<'_>) -> rusqlite::Result<ProxyRecord> {
    Ok(ProxyRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        host: row.get(3)?,
        port: row.get(4)?,
        username: row.get(5)?,
        password: row.get(6)?,
        country: row.get(7)?,
        detected_ip: row.get(8)?,
        country_code: row.get(9)?,
        region: row.get(10)?,
        city: row.get(11)?,
        timezone: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn map_ai_settings(row: &Row<'_>) -> rusqlite::Result<AiSettingsRecord> {
    Ok(AiSettingsRecord {
        id: row.get(0)?,
        provider_kind: row.get(1)?,
        base_url: row.get(2)?,
        model: row.get(3)?,
        reasoning_effort: row.get(4)?,
        review_mode: row.get(5)?,
        network_enabled: row.get(6)?,
        api_key: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_ai_model_config(row: &Row<'_>) -> rusqlite::Result<AiModelConfigRecord> {
    Ok(AiModelConfigRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_kind: row.get(2)?,
        icon: row.get(3)?,
        base_url: row.get(4)?,
        model: row.get(5)?,
        reasoning_effort: row.get(6)?,
        review_mode: row.get(7)?,
        network_enabled: row.get(8)?,
        api_key: row.get(9)?,
        is_default: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_ai_task(row: &Row<'_>) -> rusqlite::Result<AiTaskRecord> {
    Ok(AiTaskRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        model_config_id: row.get(2)?,
        prompt: row.get(3)?,
        execution_mode: row.get(4)?,
        status: row.get(5)?,
        session_id: row.get(6)?,
        cdp_url: row.get(7)?,
        requires_user_action: row.get(8)?,
        pending_approval_code: row.get(9)?,
        pending_approval_message: row.get(10)?,
        human_verification_policy: row.get(11)?,
        payment_policy: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn map_ai_task_event(row: &Row<'_>) -> rusqlite::Result<AiTaskEventRecord> {
    Ok(AiTaskEventRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        actor: row.get(2)?,
        kind: row.get(3)?,
        action: row.get(4)?,
        message: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn map_ai_audit_log(row: &Row<'_>) -> rusqlite::Result<AiAuditLogRecord> {
    Ok(AiAuditLogRecord {
        id: row.get(0)?,
        task_id: row.get(1)?,
        profile_id: row.get(2)?,
        profile_name: row.get(3)?,
        actor: row.get(4)?,
        kind: row.get(5)?,
        action: row.get(6)?,
        message: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn map_ai_profile_fact(row: &Row<'_>) -> rusqlite::Result<AiProfileFactRecord> {
    Ok(AiProfileFactRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        key_name: row.get(2)?,
        value_text: row.get(3)?,
        source: row.get(4)?,
        sensitivity: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn map_browser_extension(row: &Row<'_>) -> rusqlite::Result<BrowserExtensionRecord> {
    Ok(BrowserExtensionRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        source: row.get(2)?,
        store_url: row.get(3)?,
        store_id: row.get(4)?,
        version: row.get(5)?,
        install_path: row.get(6)?,
        default_enabled: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::open_in_memory("test-key").expect("open in-memory db")
    }

    fn sample(name: &str) -> CreateProfile {
        CreateProfile {
            name: name.to_string(),
            avatar_color: "#5640C9".to_string(),
            group_id: None,
            tag_ids: vec!["tag-1".to_string()],
            proxy_id: None,
            os: "windows".to_string(),
            platform: "Storefront".to_string(),
            browser_version: "120".to_string(),
            notes: String::new(),
            startup_page: "veil".to_string(),
            startup_url: String::new(),
            fingerprint: veil_core::Fingerprint::default(),
        }
    }

    #[test]
    fn creates_and_lists_profiles() {
        let db = db();
        let created = db.create_profile(sample("Default")).unwrap();
        assert_eq!(created.name, "Default");
        assert_eq!(created.tag_ids, vec!["tag-1".to_string()]);
        assert_eq!(created.status, "idle");

        let all = db.list_profiles().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, created.id);
    }

    #[test]
    fn updates_and_deletes_a_profile() {
        let db = db();
        let created = db.create_profile(sample("Old")).unwrap();
        let updated = db
            .update_profile(
                &created.id,
                UpdateProfile {
                    name: "New".to_string(),
                    group_id: None,
                    tag_ids: vec![],
                    proxy_id: None,
                    os: "macos".to_string(),
                    platform: "Ads".to_string(),
                    browser_version: "121".to_string(),
                    notes: "edited".to_string(),
                    startup_page: "bing".to_string(),
                    startup_url: String::new(),
                    fingerprint: veil_core::Fingerprint::default(),
                },
            )
            .unwrap();
        assert_eq!(updated.name, "New");
        assert_eq!(updated.os, "macos");
        assert_eq!(updated.startup_page, "bing");
        assert!(updated.tag_ids.is_empty());

        db.delete_profile(&created.id).unwrap();
        assert!(matches!(
            db.get_profile(&created.id),
            Err(DatabaseError::NotFound)
        ));
    }

    #[test]
    fn persists_fingerprint_round_trip() {
        let db = db();
        let mut create = sample("FP");
        create.fingerprint = veil_core::Fingerprint {
            user_agent: "UA/1".to_string(),
            locale: "en-US".to_string(),
            languages: vec!["en-US".to_string()],
            hardware_concurrency: 8,
            ..Default::default()
        };
        let saved = db.create_profile(create).unwrap();
        assert_eq!(saved.fingerprint.user_agent, "UA/1");
        let reloaded = db.get_profile(&saved.id).unwrap();
        assert_eq!(reloaded.fingerprint.hardware_concurrency, 8);
        assert_eq!(reloaded.fingerprint.languages, vec!["en-US".to_string()]);
    }

    #[test]
    fn tracks_launch_status() {
        let db = db();
        let created = db.create_profile(sample("Run")).unwrap();
        db.set_profile_status(&created.id, "running", true).unwrap();
        let after = db.get_profile(&created.id).unwrap();
        assert_eq!(after.status, "running");
        assert!(after.last_launched_at.is_some());
    }

    #[test]
    fn creates_and_lists_groups() {
        let db = db();
        let group = db
            .create_group(CreateGroup {
                name: "Team".to_string(),
                color: Some("#18935A".to_string()),
            })
            .unwrap();
        assert_eq!(group.name, "Team");
        assert_eq!(db.list_groups().unwrap().len(), 1);
        db.delete_group(&group.id).unwrap();
        assert!(db.list_groups().unwrap().is_empty());
    }

    #[test]
    fn local_api_settings_and_keys_round_trip() {
        let db = db();
        assert!(matches!(
            db.get_local_api_settings(),
            Err(DatabaseError::NotFound)
        ));

        let settings = db
            .upsert_local_api_settings(UpsertLocalApiSettings {
                enabled: true,
                port: 17688,
                bind_mode: "loopback".to_string(),
            })
            .unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.port, 17688);
        assert_eq!(settings.bind_mode, "loopback");

        let key = db
            .create_local_api_key(CreateLocalApiKey {
                name: "Default".to_string(),
                key_secret: "veil_secret".to_string(),
                suffix: "secret".to_string(),
            })
            .unwrap();
        assert_eq!(key.name, "Default");
        assert_eq!(key.suffix, "secret");
        assert_eq!(db.list_local_api_keys().unwrap().len(), 1);

        let rotated = db
            .rotate_local_api_key(&key.id, "veil_rotated".to_string(), "otated".to_string())
            .unwrap();
        assert_eq!(rotated.key_secret, "veil_rotated");
        assert_eq!(rotated.suffix, "otated");
    }

    #[test]
    fn profile_cookie_and_credentials_round_trip() {
        let db = db();
        let profile = db.create_profile(sample("Data")).unwrap();
        let empty = db.get_profile_cookie_jar(&profile.id).unwrap();
        assert_eq!(empty.cookies_json, "[]");

        let cookies = db
            .upsert_profile_cookie_jar(
                &profile.id,
                r#"[{"name":"sid","value":"secret"}]"#.to_string(),
            )
            .unwrap();
        assert!(cookies.cookies_json.contains("sid"));

        let credential = db
            .create_profile_credential(CreateProfileCredential {
                profile_id: profile.id.clone(),
                platform: "example.com".to_string(),
                username: "user".to_string(),
                password: "secret".to_string(),
                notes: String::new(),
            })
            .unwrap();
        assert_eq!(credential.platform, "example.com");
        assert_eq!(db.list_profile_credentials(&profile.id).unwrap().len(), 1);

        let updated = db
            .update_profile_credential(
                &credential.id,
                UpdateProfileCredential {
                    platform: "example.org".to_string(),
                    username: "new-user".to_string(),
                    password: "new-secret".to_string(),
                    notes: "updated".to_string(),
                },
            )
            .unwrap();
        assert_eq!(updated.platform, "example.org");
        assert_eq!(updated.password, "new-secret");

        db.delete_profile_credential(&credential.id).unwrap();
        assert!(db.list_profile_credentials(&profile.id).unwrap().is_empty());
    }

    fn proxy_sample(name: &str) -> CreateProxy {
        CreateProxy {
            name: name.to_string(),
            kind: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: Some("user".to_string()),
            password: Some("secret".to_string()),
            country: Some("US".to_string()),
        }
    }

    #[test]
    fn creates_and_lists_proxies_in_name_order() {
        let db = db();
        let bravo = db.create_proxy(proxy_sample("Bravo")).unwrap();
        let alpha = db.create_proxy(proxy_sample("Alpha")).unwrap();
        let all = db.list_proxies().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, alpha.id);
        assert_eq!(all[1].id, bravo.id);
        assert_eq!(all[0].password.as_deref(), Some("secret"));
    }

    #[test]
    fn update_proxy_preserves_password_when_none() {
        let db = db();
        let created = db.create_proxy(proxy_sample("P")).unwrap();
        let updated = db
            .update_proxy(
                &created.id,
                UpdateProxy {
                    name: "P2".to_string(),
                    kind: "socks5".to_string(),
                    host: "10.0.0.1".to_string(),
                    port: 1080,
                    username: Some("user2".to_string()),
                    password: None,
                    country: None,
                },
            )
            .unwrap();
        assert_eq!(updated.name, "P2");
        assert_eq!(updated.kind, "socks5");
        assert_eq!(updated.port, 1080);
        assert_eq!(updated.password.as_deref(), Some("secret"));
    }

    #[test]
    fn update_proxy_replaces_password_when_some() {
        let db = db();
        let created = db.create_proxy(proxy_sample("P")).unwrap();
        let updated = db
            .update_proxy(
                &created.id,
                UpdateProxy {
                    name: created.name.clone(),
                    kind: created.kind.clone(),
                    host: created.host.clone(),
                    port: created.port,
                    username: created.username.clone(),
                    password: Some(String::new()),
                    country: created.country.clone(),
                },
            )
            .unwrap();
        assert_eq!(updated.password.as_deref(), Some(""));
    }

    #[test]
    fn updates_proxy_location_metadata() {
        let db = db();
        let created = db.create_proxy(proxy_sample("P")).unwrap();
        let updated = db
            .update_proxy_location(
                &created.id,
                ProxyLocation {
                    detected_ip: Some("203.0.113.10".to_string()),
                    country: Some("Japan".to_string()),
                    country_code: Some("JP".to_string()),
                    region: Some("Tokyo".to_string()),
                    city: Some("Tokyo".to_string()),
                    timezone: Some("Asia/Tokyo".to_string()),
                },
            )
            .unwrap();

        assert_eq!(updated.detected_ip.as_deref(), Some("203.0.113.10"));
        assert_eq!(updated.country.as_deref(), Some("Japan"));
        assert_eq!(updated.country_code.as_deref(), Some("JP"));
        assert_eq!(updated.city.as_deref(), Some("Tokyo"));
        assert_eq!(updated.timezone.as_deref(), Some("Asia/Tokyo"));
    }

    #[test]
    fn deletes_proxy_and_clears_profile_assignment() {
        let db = db();
        let proxy = db.create_proxy(proxy_sample("P")).unwrap();
        let mut create = sample("Linked");
        create.proxy_id = Some(proxy.id.clone());
        let profile = db.create_profile(create).unwrap();
        assert_eq!(profile.proxy_id.as_deref(), Some(proxy.id.as_str()));

        db.delete_proxy(&proxy.id).unwrap();

        let after = db.get_profile(&profile.id).unwrap();
        assert!(
            after.proxy_id.is_none(),
            "FK ON DELETE SET NULL should clear the link"
        );
        assert!(matches!(
            db.get_proxy(&proxy.id),
            Err(DatabaseError::NotFound)
        ));
    }

    #[test]
    fn set_profile_proxy_round_trip() {
        let db = db();
        let proxy = db.create_proxy(proxy_sample("P")).unwrap();
        let profile = db.create_profile(sample("F")).unwrap();

        db.set_profile_proxy(&profile.id, Some(&proxy.id)).unwrap();
        let after = db.get_profile(&profile.id).unwrap();
        assert_eq!(after.proxy_id.as_deref(), Some(proxy.id.as_str()));

        db.set_profile_proxy(&profile.id, None).unwrap();
        let cleared = db.get_profile(&profile.id).unwrap();
        assert!(cleared.proxy_id.is_none());

        assert!(matches!(
            db.set_profile_proxy("nope", Some(&proxy.id)),
            Err(DatabaseError::NotFound)
        ));
    }

    #[test]
    fn ai_settings_and_tasks_round_trip() {
        let db = db();
        let settings = db
            .upsert_ai_settings(UpsertAiSettings {
                provider_kind: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.5".to_string(),
                reasoning_effort: "medium".to_string(),
                review_mode: "reviewed".to_string(),
                network_enabled: true,
                api_key: Some("secret".to_string()),
            })
            .unwrap();
        assert_eq!(settings.model, "gpt-5.5");

        let profile = db.create_profile(sample("Agent")).unwrap();
        let task = db
            .create_ai_task(CreateAiTask {
                profile_id: profile.id.clone(),
                model_config_id: None,
                prompt: "Register account".to_string(),
                execution_mode: "reviewed".to_string(),
                status: "running".to_string(),
                session_id: Some("session-1".to_string()),
                cdp_url: Some("http://127.0.0.1:9222".to_string()),
                requires_user_action: false,
                pending_approval_code: None,
                pending_approval_message: None,
                human_verification_policy: "retryOnceThenPause".to_string(),
                payment_policy: "userRequired".to_string(),
            })
            .unwrap();
        assert_eq!(task.profile_id, profile.id);

        let event = db
            .create_ai_task_event(CreateAiTaskEvent {
                task_id: task.id.clone(),
                actor: "system".to_string(),
                kind: "info".to_string(),
                action: "event".to_string(),
                message: "started".to_string(),
            })
            .unwrap();
        assert_eq!(event.task_id, task.id);

        let fact = db
            .upsert_ai_profile_fact(UpsertAiProfileFact {
                profile_id: profile.id.clone(),
                key_name: "gmail.email".to_string(),
                value_text: "user@gmail.com".to_string(),
                source: "manual".to_string(),
                sensitivity: "sensitive".to_string(),
            })
            .unwrap();
        assert_eq!(fact.key_name, "gmail.email");
        assert_eq!(db.list_ai_task_events(&task.id).unwrap().len(), 1);
        assert_eq!(db.list_ai_profile_facts(&profile.id).unwrap().len(), 1);
        assert_eq!(db.list_ai_tasks_for_profile(&profile.id).unwrap().len(), 1);
    }
}
