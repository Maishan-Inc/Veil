use serde::{Deserialize, Serialize};

/// A persisted proxy row. The password is stored as-is and protected by the surrounding
/// SQLCipher encryption; it is NEVER exposed to the IPC layer (the service maps it to
/// `hasPassword: bool` before crossing the boundary).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyRecord {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country: Option<String>,
    pub detected_ip: Option<String>,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateProxy {
    pub name: String,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country: Option<String>,
}

/// Update is full-field except for `password`, where `None` means "leave the existing
/// password untouched" and `Some(_)` replaces it (use `Some(String::new())` to clear).
#[derive(Debug, Clone)]
pub struct UpdateProxy {
    pub name: String,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProxyLocation {
    pub detected_ip: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
}
