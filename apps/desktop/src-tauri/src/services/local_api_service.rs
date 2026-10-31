use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use veil_core::{AppError, AppResult, ErrorCode};
use veil_database::{
    CreateLocalApiKey, Database, DatabaseError, LocalApiKeyRecord, LocalApiSettingsRecord,
    UpsertLocalApiSettings,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiSettings {
    pub enabled: bool,
    pub port: u16,
    pub bind_mode: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateLocalApiSettingsInput {
    pub enabled: bool,
    pub port: u16,
    pub bind_mode: String,
}

impl Default for UpdateLocalApiSettingsInput {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            bind_mode: DEFAULT_BIND_MODE.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiKey {
    pub id: String,
    pub name: String,
    pub suffix: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiKeyIssue {
    pub key: LocalApiKey,
    pub full_key: String,
}

#[derive(Clone)]
pub struct LocalApiService {
    db: Arc<Mutex<Database>>,
}

impl LocalApiService {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Database> {
        self.db.lock().expect("database lock poisoned")
    }

    pub fn settings(&self) -> AppResult<LocalApiSettings> {
        match self.db().get_local_api_settings() {
            Ok(record) => Ok(LocalApiSettings::from(record)),
            Err(DatabaseError::NotFound) => Ok(LocalApiSettings::default()),
            Err(err) => Err(map_read_error(err)),
        }
    }

    pub fn update_settings(
        &self,
        input: UpdateLocalApiSettingsInput,
    ) -> AppResult<LocalApiSettings> {
        validate_port(input.port)?;
        let bind_mode = normalize_bind_mode(&input.bind_mode);
        let record = self
            .db()
            .upsert_local_api_settings(UpsertLocalApiSettings {
                enabled: input.enabled,
                port: input.port,
                bind_mode,
            })
            .map_err(map_write_error)?;
        Ok(LocalApiSettings::from(record))
    }

    pub fn list_keys(&self) -> AppResult<Vec<LocalApiKey>> {
        let records = self.db().list_local_api_keys().map_err(map_read_error)?;
        Ok(records.into_iter().map(LocalApiKey::from).collect())
    }

    pub fn generate_key(&self, name: String) -> AppResult<LocalApiKeyIssue> {
        let full_key = generate_key_secret();
        let suffix = key_suffix(&full_key);
        let key_name = normalize_key_name(&name);
        let record = self
            .db()
            .create_local_api_key(CreateLocalApiKey {
                name: key_name,
                key_secret: full_key.clone(),
                suffix,
            })
            .map_err(map_write_error)?;
        Ok(LocalApiKeyIssue {
            key: LocalApiKey::from(record),
            full_key,
        })
    }

    pub fn rotate_key(&self, key_id: String) -> AppResult<LocalApiKeyIssue> {
        let full_key = generate_key_secret();
        let suffix = key_suffix(&full_key);
        let record = self
            .db()
            .rotate_local_api_key(&key_id, full_key.clone(), suffix)
            .map_err(|err| match err {
                DatabaseError::NotFound => AppError::from(ErrorCode::LocalApiKeyNotFound),
                other => map_write_error(other),
            })?;
        Ok(LocalApiKeyIssue {
            key: LocalApiKey::from(record),
            full_key,
        })
    }

    /// Authenticate a full key secret against the stored keys — the local HTTP listener's
    /// bearer-token check. Returns the matching key and records its use so the console can
    /// show `last_used_at`.
    pub fn authenticate(&self, full_key: &str) -> AppResult<LocalApiKey> {
        let record = self
            .db()
            .find_local_api_key_by_secret(full_key)
            .map_err(|_| AppError::from(ErrorCode::LocalApiKeyNotFound))?;
        let _ = self.db().touch_local_api_key_used(&record.id);
        Ok(LocalApiKey::from(record))
    }
}

impl Default for LocalApiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            bind_mode: DEFAULT_BIND_MODE.to_string(),
            updated_at: None,
        }
    }
}

impl From<LocalApiSettingsRecord> for LocalApiSettings {
    fn from(record: LocalApiSettingsRecord) -> Self {
        Self {
            enabled: record.enabled,
            port: record.port,
            bind_mode: record.bind_mode,
            updated_at: Some(record.updated_at),
        }
    }
}

impl From<LocalApiKeyRecord> for LocalApiKey {
    fn from(record: LocalApiKeyRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            suffix: record.suffix,
            created_at: record.created_at,
            updated_at: record.updated_at,
            last_used_at: record.last_used_at,
        }
    }
}

pub const DEFAULT_PORT: u16 = 17688;
pub const DEFAULT_BIND_MODE: &str = "loopback";
const MIN_USER_PORT: u16 = 1024;

pub fn requires_external_access_verification(
    previous: &LocalApiSettings,
    next: &UpdateLocalApiSettingsInput,
) -> bool {
    previous.bind_mode != "lan" && normalize_bind_mode(&next.bind_mode) == "lan"
}

fn validate_port(port: u16) -> AppResult<()> {
    if port < MIN_USER_PORT {
        return Err(ErrorCode::LocalApiInvalidPort.into());
    }
    Ok(())
}

fn normalize_bind_mode(value: &str) -> String {
    match value {
        "lan" => "lan".to_string(),
        _ => DEFAULT_BIND_MODE.to_string(),
    }
}

fn normalize_key_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Default".to_string()
    } else {
        trimmed.to_string()
    }
}

fn generate_key_secret() -> String {
    format!(
        "veil_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn key_suffix(value: &str) -> String {
    value
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn map_read_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::LocalApiKeyNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseReadFailed.into()
        }
    }
}

fn map_write_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::LocalApiKeyNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseWriteFailed.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> LocalApiService {
        let db = Database::open_in_memory("test-key").unwrap();
        LocalApiService::new(Arc::new(Mutex::new(db)))
    }

    #[test]
    fn default_settings_are_safe() {
        let settings = service().settings().unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.port, DEFAULT_PORT);
        assert_eq!(settings.bind_mode, "loopback");
    }

    #[test]
    fn validates_and_persists_settings() {
        let service = service();
        assert_eq!(
            service
                .update_settings(UpdateLocalApiSettingsInput {
                    enabled: true,
                    port: 80,
                    bind_mode: "loopback".to_string(),
                })
                .unwrap_err()
                .code(),
            ErrorCode::LocalApiInvalidPort
        );

        let saved = service
            .update_settings(UpdateLocalApiSettingsInput {
                enabled: true,
                port: 17688,
                bind_mode: "lan".to_string(),
            })
            .unwrap();
        assert!(saved.enabled);
        assert_eq!(saved.bind_mode, "lan");
    }

    #[test]
    fn issues_and_rotates_keys() {
        let service = service();
        let issued = service.generate_key(String::new()).unwrap();
        assert_eq!(issued.key.name, "Default");
        assert!(issued.full_key.starts_with("veil_"));
        assert_eq!(issued.key.suffix, key_suffix(&issued.full_key));

        let rotated = service.rotate_key(issued.key.id.clone()).unwrap();
        assert_ne!(rotated.full_key, issued.full_key);
        assert_eq!(rotated.key.id, issued.key.id);
    }

    #[test]
    fn detects_external_access_transition() {
        let previous = LocalApiSettings::default();
        let next = UpdateLocalApiSettingsInput {
            enabled: true,
            port: DEFAULT_PORT,
            bind_mode: "lan".to_string(),
        };
        assert!(requires_external_access_verification(&previous, &next));
    }

    #[test]
    fn authenticates_an_issued_key_and_tracks_its_use() {
        let service = service();
        let issued = service.generate_key(String::new()).unwrap();

        let key = service.authenticate(&issued.full_key).unwrap();
        assert_eq!(key.id, issued.key.id);
        assert_eq!(key.name, "Default");

        let refreshed = service.authenticate(&issued.full_key).unwrap();
        assert!(refreshed.last_used_at.is_some());
    }

    #[test]
    fn rejects_an_unknown_key() {
        let service = service();
        let err = service.authenticate("veil_does_not_exist").unwrap_err();
        assert_eq!(err.code(), ErrorCode::LocalApiKeyNotFound);
    }
}
