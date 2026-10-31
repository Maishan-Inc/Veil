use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalApiSettingsRecord {
    pub id: String,
    pub enabled: bool,
    pub port: u16,
    pub bind_mode: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UpsertLocalApiSettings {
    pub enabled: bool,
    pub port: u16,
    pub bind_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalApiKeyRecord {
    pub id: String,
    pub name: String,
    pub key_secret: String,
    pub suffix: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateLocalApiKey {
    pub name: String,
    pub key_secret: String,
    pub suffix: String,
}
