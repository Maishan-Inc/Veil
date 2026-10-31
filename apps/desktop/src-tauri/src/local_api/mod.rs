//! The local automation API — a loopback HTTP listener that exposes the same services the
//! Tauri commands do, guarded by a generated API key.
//!
//! The handlers below are the contract. The listener is off until the user enables it; it binds
//! `127.0.0.1` (loopback) or `0.0.0.0` (`lan`, gated by sensitive verification in the UI).
//!
//! The server thread upgrades a weak handle to [`AppState`] on every request. The managed
//! state outlives every request, so the upgrade always succeeds while the app runs; the
//! weak handle is what lets `setup` keep the strong `Arc` from outliving Tauri's own copy.

mod handlers;
mod http;
mod router;

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use veil_core::{AppError, AppResult, ErrorCode};

use crate::app_state::AppState;
use crate::services::LocalApiSettings;

pub use router::dispatch;

const SENSITIVE_TOKEN_TTL: Duration = Duration::from_secs(120);

struct SensitiveToken {
    key_id: String,
    expires_at: Instant,
}

/// Ephemeral, one-use credentials, scoped to the bearer key that requested them.
#[derive(Default)]
pub(crate) struct SensitiveTokenStore(Mutex<HashMap<String, SensitiveToken>>);

impl SensitiveTokenStore {
    pub(crate) fn issue(&self, key_id: &str) -> String {
        let mut tokens = self.0.lock().expect("sensitive token lock poisoned");
        tokens.retain(|_, entry| entry.expires_at > Instant::now());
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        tokens.insert(
            token.clone(),
            SensitiveToken {
                key_id: key_id.to_string(),
                expires_at: Instant::now() + SENSITIVE_TOKEN_TTL,
            },
        );
        token
    }

    pub(crate) fn consume(&self, token: &str, key_id: &str) -> bool {
        let mut tokens = self.0.lock().expect("sensitive token lock poisoned");
        tokens.retain(|_, entry| entry.expires_at > Instant::now());
        tokens
            .remove(token)
            .is_some_and(|entry| entry.key_id == key_id)
    }
}

/// Owns the local API listener thread. Managed alongside [`AppState`] so commands can
/// reconcile it with settings edits (and the vault-unlock that makes settings readable).
pub struct LocalApiServer {
    inner: Arc<LocalApiInner>,
}

struct LocalApiInner {
    state: Weak<AppState>,
    addr: Mutex<Option<SocketAddr>>,
    shutdown: AtomicBool,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl LocalApiServer {
    pub fn new(state: Weak<AppState>) -> Self {
        Self {
            inner: Arc::new(LocalApiInner {
                state,
                addr: Mutex::new(None),
                shutdown: AtomicBool::new(true),
                handle: Mutex::new(None),
            }),
        }
    }

    /// Reconcile the listener with the persisted settings. Called at startup and after any
    /// event that can unlock the database — a locked vault means no settings and no data,
    /// so the listener stays down rather than serving 503s.
    pub fn reconcile(&self, state: &AppState) {
        match state.local_api().and_then(|service| service.settings()) {
            Ok(settings) => {
                let _ = self.apply_settings(&settings);
            }
            Err(_) => self.stop(),
        }
    }

    /// Start, restart or stop the listener so it matches `settings`. Fails with
    /// [`ErrorCode::LocalApiBindFailed`] when the address cannot be bound (e.g. port in
    /// use), which the caller surfaces to the user.
    pub fn apply_settings(&self, settings: &LocalApiSettings) -> AppResult<()> {
        if !settings.enabled {
            self.stop();
            return Ok(());
        }
        let addr = bind_address(settings)?;
        self.restart(addr)
    }

    fn restart(&self, addr: SocketAddr) -> AppResult<()> {
        let already_running = {
            let addr_guard = self.inner.addr.lock().expect("addr lock poisoned");
            *addr_guard == Some(addr)
                && self
                    .inner
                    .handle
                    .lock()
                    .expect("handle lock poisoned")
                    .is_some()
        };
        if already_running {
            return Ok(());
        }

        self.stop();

        let listener =
            TcpListener::bind(addr).map_err(|_| AppError::from(ErrorCode::LocalApiBindFailed))?;
        listener.set_nonblocking(true).ok();

        let inner = Arc::clone(&self.inner);
        self.inner.shutdown.store(false, Ordering::SeqCst);
        let handle = std::thread::Builder::new()
            .name("veil-local-api".to_string())
            .spawn(move || accept_loop(inner, listener))
            .map_err(|_| AppError::from(ErrorCode::LocalApiBindFailed))?;

        *self.inner.addr.lock().expect("addr lock poisoned") = Some(addr);
        *self.inner.handle.lock().expect("handle lock poisoned") = Some(handle);
        Ok(())
    }

    pub fn stop(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self
            .inner
            .handle
            .lock()
            .expect("handle lock poisoned")
            .take()
        {
            let _ = handle.join();
        }
        *self.inner.addr.lock().expect("addr lock poisoned") = None;
    }
}

impl Drop for LocalApiServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The response envelope is shared by the router, the handlers and the accept loop.
pub(crate) mod envelope {
    use serde_json::json;

    pub fn ok(data: serde_json::Value) -> serde_json::Value {
        json!({ "success": true, "data": data })
    }

    pub fn error(code: &str, message: &str) -> serde_json::Value {
        json!({ "success": false, "error": { "code": code, "message": message } })
    }
}

fn bind_address(settings: &LocalApiSettings) -> AppResult<SocketAddr> {
    let host = if settings.bind_mode == "lan" {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    format!("{host}:{}", settings.port)
        .parse()
        .map_err(|_| AppError::from(ErrorCode::LocalApiInvalidPort))
}

fn accept_loop(inner: Arc<LocalApiInner>, listener: TcpListener) {
    loop {
        if inner.shutdown.load(Ordering::SeqCst) {
            break;
        }
        match listener.accept() {
            Ok((stream, _peer)) => {
                let inner = Arc::clone(&inner);
                std::thread::spawn(move || handle_connection(&inner, stream));
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(inner: &Arc<LocalApiInner>, mut stream: TcpStream) {
    let response = match http::read_request(&mut stream) {
        Ok(request) => match inner.state.upgrade() {
            Some(state) => dispatch(&state, &request),
            None => http::Response::json(
                503,
                &envelope::error("localApi.unavailable", "Local API is shutting down."),
            ),
        },
        Err(_) => http::Response::json(
            400,
            &envelope::error("localApi.badRequest", "Malformed HTTP request."),
        ),
    };
    let _ = response.write_to(&mut stream);
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// Map an [`AppError`] to an HTTP status + `{ success, error }` body. The `code` is the
/// stable dotted string; the `message` is a plain-English description for automation
/// clients (the UI localizes through i18n instead).
pub(crate) fn app_error_response(err: AppError) -> http::Response {
    let code = err.code().as_str();
    http::Response::json(
        http_status_for(code),
        &envelope::error(code, message_for(code)),
    )
}

fn http_status_for(code: &str) -> u16 {
    match code {
        "profile.notFound" | "extension.notFound" | "proxy.notFound" | "localApi.keyNotFound" => {
            404
        }
        "security.verificationFailed" => 403,
        "storage.databaseReadFailed"
        | "storage.databaseWriteFailed"
        | "storage.encryptionFailed"
        | "storage.decryptionFailed"
        | "storage.locked" => 500,
        "app.unknown" => 500,
        _ => 400,
    }
}

fn message_for(code: &str) -> &'static str {
    match code {
        "profile.notFound" => "Profile was not found.",
        "profile.nameRequired" => "Profile name is required.",
        "proxy.notFound" => "Proxy was not found.",
        "extension.notFound" => "Extension was not found.",
        "fingerprint.invalid" => "Fingerprint configuration is invalid.",
        "localApi.badRequest" => "Malformed request body.",
        "localApi.bindFailed" => "The API listener could not bind to that address.",
        "localApi.invalidPort" => "Use a port from 1024 to 65535.",
        "localApi.keyNotFound" => "API key was not found.",
        "security.verificationFailed" => "Sensitive verification failed.",
        "storage.locked" => "The local vault is locked.",
        "storage.databaseReadFailed" => "Could not read local data.",
        "storage.databaseWriteFailed" => "Could not write local data.",
        "app.unknown" => "The request failed.",
        _ => "The request failed.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_lan_bind_addresses() {
        let settings = |bind_mode: &str, port: u16| LocalApiSettings {
            enabled: true,
            port,
            bind_mode: bind_mode.to_string(),
            updated_at: None,
        };
        assert_eq!(
            bind_address(&settings("loopback", 17688)).unwrap(),
            "127.0.0.1:17688".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            bind_address(&settings("lan", 17688)).unwrap(),
            "0.0.0.0:17688".parse::<SocketAddr>().unwrap()
        );
        // Any unknown bind mode collapses to loopback.
        assert_eq!(
            bind_address(&settings("public", 17688)).unwrap(),
            "127.0.0.1:17688".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn error_status_and_message_mapping() {
        assert_eq!(http_status_for("profile.notFound"), 404);
        assert_eq!(http_status_for("security.verificationFailed"), 403);
        assert_eq!(http_status_for("storage.locked"), 500);
        assert_eq!(message_for("profile.notFound"), "Profile was not found.");
        assert_eq!(
            message_for("localApi.badRequest"),
            "Malformed request body."
        );
    }

    #[test]
    fn envelope_shapes_match_the_api_contract() {
        let ok = envelope::ok(serde_json::json!({ "profileId": "p1" }));
        assert_eq!(ok["success"], serde_json::json!(true));
        assert!(ok.get("data").is_some());

        let err = envelope::error("profile.notFound", "Profile was not found.");
        assert_eq!(err["success"], serde_json::json!(false));
        assert_eq!(err["error"]["code"], serde_json::json!("profile.notFound"));
        assert_eq!(
            err["error"]["message"],
            serde_json::json!("Profile was not found.")
        );
    }

    #[test]
    fn sensitive_tokens_are_key_scoped_one_use_and_short_lived() {
        let store = SensitiveTokenStore::default();
        let token = store.issue("key-a");
        assert!(!store.consume(&token, "key-b"));
        assert!(!store.consume(&token, "key-a"));

        let token = store.issue("key-a");
        assert!(store.consume(&token, "key-a"));
        assert!(!store.consume(&token, "key-a"));

        let token = store.issue("key-a");
        store.0.lock().unwrap().get_mut(&token).unwrap().expires_at = Instant::now();
        assert!(!store.consume(&token, "key-a"));
    }

    #[test]
    fn server_starts_and_stops_on_an_ephemeral_port() {
        // A Weak::new() holds no AppState: the accept loop never handles a request, but
        // starting and stopping the listener exercises the full lifecycle without a DB.
        let server = LocalApiServer::new(Weak::new());
        let settings = LocalApiSettings {
            enabled: true,
            port: 0,
            bind_mode: "loopback".to_string(),
            updated_at: None,
        };
        server.apply_settings(&settings).unwrap();
        // Applying the same settings again is a no-op, not a double-bind.
        server.apply_settings(&settings).unwrap();
        server.stop();

        // Disabling stops the listener; restarting after stop works.
        let mut disabled = settings.clone();
        disabled.enabled = false;
        server.apply_settings(&disabled).unwrap();
        server.apply_settings(&settings).unwrap();
        server.stop();
    }
}
