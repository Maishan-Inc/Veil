use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use veil_core::{AppError, AppResult, ErrorCode};
use veil_database::{
    CreateProxy, Database, DatabaseError, ProxyLocation, ProxyRecord, UpdateProxy,
};
use veil_runtime::ProxyTestOutcome;

/// Proxy DTO sent to the UI. The password itself never crosses this boundary — the
/// service exposes `has_password: bool` only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub has_password: bool,
    pub country: Option<String>,
    pub detected_ip: Option<String>,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

impl From<ProxyRecord> for Proxy {
    fn from(r: ProxyRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            proxy_type: r.kind,
            host: r.host,
            port: r.port,
            username: r.username,
            has_password: r.password.as_deref().is_some_and(|s| !s.is_empty()),
            country: r.country,
            detected_ip: r.detected_ip,
            country_code: r.country_code,
            region: r.region,
            city: r.city,
            timezone: r.timezone,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

impl From<ProxyRecord> for ProxyCredentials {
    fn from(record: ProxyRecord) -> Self {
        Self {
            username: record.username,
            password: record.password,
        }
    }
}

/// Field naming: the frontend uses `type` (matches `@veil/shared::Proxy`); on the wire
/// serde renames the back-end `proxy_type` to match.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CreateProxyInput {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country: Option<String>,
}

impl Default for CreateProxyInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            proxy_type: "http".to_string(),
            host: String::new(),
            port: 0,
            username: None,
            password: None,
            country: None,
        }
    }
}

/// `password`:
/// - `None` — leave the existing password untouched.
/// - `Some(non-empty)` — replace with this value.
/// - `Some(empty)` — explicitly clear the password.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateProxyInput {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country: Option<String>,
}

impl Default for UpdateProxyInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            proxy_type: "http".to_string(),
            host: String::new(),
            port: 0,
            username: None,
            password: None,
            country: None,
        }
    }
}

/// Mirror of `@veil/shared::ProxyTestResult`. `ok=true` carries the discovered IP,
/// country, and round-trip latency in ms; `ok=false` carries a stable `code` the UI
/// localizes through the i18n catalog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub ok: bool,
    pub ip: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
    pub latency_ms: Option<u64>,
    pub code: Option<String>,
}

impl ProxyTestResult {
    pub fn success(outcome: ProxyTestOutcome) -> Self {
        Self {
            ok: true,
            ip: outcome.ip,
            country: outcome.country,
            country_code: outcome.country_code,
            region: outcome.region,
            city: outcome.city,
            timezone: outcome.timezone,
            latency_ms: outcome.latency_ms,
            code: None,
        }
    }

    pub fn failure(code: &'static str) -> Self {
        Self {
            ok: false,
            ip: None,
            country: None,
            country_code: None,
            region: None,
            city: None,
            timezone: None,
            latency_ms: None,
            code: Some(code.to_string()),
        }
    }
}

const VALID_KINDS: [&str; 3] = ["http", "https", "socks5"];

fn validate(name: &str, kind: &str, host: &str, port: u16) -> AppResult<()> {
    if name.trim().is_empty() || host.trim().is_empty() {
        return Err(ErrorCode::ProxyInvalid.into());
    }
    if port == 0 {
        return Err(ErrorCode::ProxyInvalid.into());
    }
    if !VALID_KINDS.contains(&kind) {
        return Err(ErrorCode::ProxyInvalid.into());
    }
    Ok(())
}

fn map_read_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::ProxyNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseReadFailed.into()
        }
    }
}

fn map_write_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::ProxyNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseWriteFailed.into()
        }
    }
}

/// SQLCipher-backed proxy library. Shares the database connection with `ProfileService`
/// through `Arc<Mutex<Database>>`; all reads/writes serialize on the same mutex.
#[derive(Clone)]
pub struct ProxyService {
    db: Arc<Mutex<Database>>,
}

impl ProxyService {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Database> {
        self.db.lock().expect("database lock poisoned")
    }

    pub fn list(&self) -> AppResult<Vec<Proxy>> {
        let records = self.db().list_proxies().map_err(map_read_error)?;
        Ok(records.into_iter().map(Proxy::from).collect())
    }

    pub fn get(&self, id: &str) -> AppResult<Proxy> {
        let record = self.db().get_proxy(id).map_err(map_read_error)?;
        Ok(Proxy::from(record))
    }

    /// Reads the proxy record **including the plaintext password**. Used by the test_proxy
    /// command and (in a later phase) by the launch path when proxy auth wiring lands.
    /// MUST NOT cross the IPC boundary.
    pub fn get_record(&self, id: &str) -> AppResult<ProxyRecord> {
        self.db().get_proxy(id).map_err(map_read_error)
    }

    pub fn update_location(&self, id: &str, outcome: &ProxyTestOutcome) -> AppResult<Proxy> {
        let record = self
            .db()
            .update_proxy_location(
                id,
                ProxyLocation {
                    detected_ip: outcome.ip.clone(),
                    country: outcome.country.clone(),
                    country_code: outcome.country_code.clone(),
                    region: outcome.region.clone(),
                    city: outcome.city.clone(),
                    timezone: outcome.timezone.clone(),
                },
            )
            .map_err(map_write_error)?;
        Ok(Proxy::from(record))
    }

    pub fn create(&self, input: CreateProxyInput) -> AppResult<Proxy> {
        validate(&input.name, &input.proxy_type, &input.host, input.port)?;
        let create = CreateProxy {
            name: input.name.trim().to_string(),
            kind: input.proxy_type,
            host: input.host.trim().to_string(),
            port: input.port,
            username: input.username.filter(|s| !s.is_empty()),
            password: input.password.filter(|s| !s.is_empty()),
            country: input.country.filter(|s| !s.is_empty()),
        };
        let record = self.db().create_proxy(create).map_err(map_write_error)?;
        Ok(Proxy::from(record))
    }

    pub fn update(&self, id: &str, input: UpdateProxyInput) -> AppResult<Proxy> {
        validate(&input.name, &input.proxy_type, &input.host, input.port)?;
        let update = UpdateProxy {
            name: input.name.trim().to_string(),
            kind: input.proxy_type,
            host: input.host.trim().to_string(),
            port: input.port,
            username: input.username.filter(|s| !s.is_empty()),
            // password: None means "leave alone"; Some("") means "clear" — both are
            // passed straight through to the database layer.
            password: input.password,
            country: input.country.filter(|s| !s.is_empty()),
        };
        let record = self
            .db()
            .update_proxy(id, update)
            .map_err(map_write_error)?;
        Ok(Proxy::from(record))
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        self.db().delete_proxy(id).map_err(map_write_error)
    }

    pub fn set_profile_proxy(&self, profile_id: &str, proxy_id: Option<&str>) -> AppResult<()> {
        self.db()
            .set_profile_proxy(profile_id, proxy_id)
            .map_err(|err| match err {
                // The bad id here is the *profile*, not the proxy.
                DatabaseError::NotFound => ErrorCode::ProfileNotFound.into(),
                DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
                    ErrorCode::StorageDatabaseWriteFailed.into()
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ProxyService {
        let db = Database::open_in_memory("test-key").unwrap();
        ProxyService::new(Arc::new(Mutex::new(db)))
    }

    fn input(name: &str) -> CreateProxyInput {
        CreateProxyInput {
            name: name.to_string(),
            proxy_type: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: Some("u".to_string()),
            password: Some("p".to_string()),
            country: None,
        }
    }

    #[test]
    fn creates_proxy_and_hides_password() {
        let svc = service();
        let proxy = svc.create(input("A")).unwrap();
        assert!(proxy.has_password);
        assert_eq!(proxy.username.as_deref(), Some("u"));
        // No `password` field on the DTO — type-checked at compile time, but the
        // backend record still has it for launch/test paths.
        let rec = svc.get_record(&proxy.id).unwrap();
        assert_eq!(rec.password.as_deref(), Some("p"));
    }

    #[test]
    fn rejects_invalid_input() {
        let svc = service();
        let mut bad = input("");
        bad.name = String::new();
        assert_eq!(svc.create(bad).unwrap_err().code(), ErrorCode::ProxyInvalid);

        let mut bad = input("A");
        bad.proxy_type = "ftp".to_string();
        assert_eq!(svc.create(bad).unwrap_err().code(), ErrorCode::ProxyInvalid);

        let mut bad = input("A");
        bad.port = 0;
        assert_eq!(svc.create(bad).unwrap_err().code(), ErrorCode::ProxyInvalid);
    }

    #[test]
    fn update_preserves_password_when_none() {
        let svc = service();
        let proxy = svc.create(input("A")).unwrap();
        let updated = svc
            .update(
                &proxy.id,
                UpdateProxyInput {
                    name: "A2".to_string(),
                    proxy_type: "https".to_string(),
                    host: "10.0.0.1".to_string(),
                    port: 443,
                    username: Some("u2".to_string()),
                    password: None,
                    country: None,
                },
            )
            .unwrap();
        assert_eq!(updated.name, "A2");
        assert!(updated.has_password, "password preserved on None");
        let rec = svc.get_record(&proxy.id).unwrap();
        assert_eq!(rec.password.as_deref(), Some("p"));
    }
}
