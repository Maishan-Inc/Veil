//! Routing, authentication and the response envelope for the local API.
//!
//! Every request is authenticated against a generated API key, then dispatched to a
//! handler. Sensitive endpoints are gated in the handlers behind local verification.

use veil_core::AppError;

use crate::app_state::AppState;

use super::envelope;
use super::http::{Request, Response};
use super::{app_error_response, handlers};

/// Auth failures are a distinct 401-shaped result, not an `AppError`: they carry codes
/// from the API contract that the app's `ErrorCode` enum does not model.
type AuthError = (&'static str, &'static str);

pub fn dispatch(state: &AppState, request: &Request) -> Response {
    let key = match authenticate(state, request) {
        Ok(key) => key,
        Err((code, message)) => return Response::json(401, &envelope::error(code, message)),
    };

    let method = request.method.as_str();
    // The base URL is `<host>/v1`; strip that prefix so routes read `["profiles"]` etc.
    let decoded = request.segments();
    let strip_v1 = decoded.first().is_some_and(|segment| segment == "v1");
    let segments: Vec<&str> = decoded
        .iter()
        .skip(usize::from(strip_v1))
        .map(String::as_str)
        .collect();

    match route(state, request, method, &segments, &key.id) {
        Ok(data) => Response::json(200, &envelope::ok(data)),
        Err(RouteError::NotFound) => Response::json(
            404,
            &envelope::error("localApi.notFound", "No such endpoint."),
        ),
        Err(RouteError::App(err)) => app_error_response(err),
    }
}

fn authenticate(
    state: &AppState,
    request: &Request,
) -> Result<crate::services::LocalApiKey, AuthError> {
    const MISSING: AuthError = ("auth.missingApiKey", "Missing API key.");
    let header = request.header("authorization").ok_or(MISSING)?;
    let token = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(MISSING)?;

    let service = state
        .local_api()
        .map_err(|_| ("localApi.unavailable", "The local vault is locked."))?;
    service
        .authenticate(token)
        .map_err(|_| ("auth.invalidApiKey", "Invalid API key."))
}

enum RouteError {
    NotFound,
    App(AppError),
}

impl From<AppError> for RouteError {
    fn from(err: AppError) -> Self {
        RouteError::App(err)
    }
}

fn route(
    state: &AppState,
    request: &Request,
    method: &str,
    segments: &[&str],
    key_id: &str,
) -> Result<serde_json::Value, RouteError> {
    let value = match (method, segments) {
        ("GET", ["health"]) => handlers::health(state)?,
        ("POST", ["sensitive-tokens"]) => handlers::issue_sensitive_token(state, request, key_id)?,
        ("GET", ["profiles"]) => handlers::list_profiles(state, request)?,
        ("POST", ["profiles"]) => handlers::create_profile(state, request)?,
        ("GET", ["profiles", id]) => handlers::get_profile(state, id)?,
        ("PATCH", ["profiles", id]) => handlers::update_profile(state, id, request)?,
        ("DELETE", ["profiles", id]) => handlers::delete_profile(state, id)?,
        ("PATCH", ["profiles", id, "fingerprint"]) => {
            handlers::update_fingerprint(state, id, request)?
        }
        ("POST", ["profiles", id, "launch"]) => handlers::launch_profile(state, id)?,
        ("POST", ["temp-profiles", "launch"]) => {
            handlers::launch_temporary_profile(state, request)?
        }
        ("POST", ["temp-profiles", "custom-launch"]) => {
            handlers::launch_custom_temporary_profile(state, request)?
        }
        ("POST", ["temp-profiles", session_id, "close"]) => {
            handlers::close_temporary_profile(state, session_id)?
        }
        ("DELETE", ["profiles", id, "data"]) => {
            handlers::delete_profile_data(state, id, request, key_id)?
        }
        ("DELETE", ["profiles", id, "cache"]) => {
            handlers::clear_profile_cache(state, id, request, key_id)?
        }
        ("GET", ["profiles", id, "running"]) => handlers::is_profile_running(state, id)?,
        ("POST", ["profiles", id, "close"]) => handlers::close_profile(state, id)?,
        ("GET", ["running-profiles"]) => handlers::running_profiles(state)?,
        ("GET", ["groups"]) => handlers::list_groups(state)?,
        ("POST", ["groups"]) => handlers::create_group(state, request)?,
        ("PATCH", ["groups", id]) => handlers::update_group(state, id, request)?,
        ("DELETE", ["groups", id]) => handlers::delete_group(state, id)?,
        ("GET", ["extensions"]) => handlers::list_extensions(state)?,
        ("POST", ["extensions"]) => handlers::import_extension(state, request)?,
        ("DELETE", ["extensions", id]) => handlers::delete_extension(state, id)?,
        ("GET", ["profiles", id, "cookies"]) => handlers::get_cookies(state, id, request, key_id)?,
        ("PUT", ["profiles", id, "cookies"]) => {
            handlers::update_cookies(state, id, request, key_id)?
        }
        ("GET", ["profiles", id, "credentials"]) => {
            handlers::list_credentials(state, id, request, key_id)?
        }
        ("POST", ["profiles", id, "credentials"]) => {
            handlers::create_credential(state, id, request, key_id)?
        }
        ("PATCH", ["profiles", id, "credentials", credential_id]) => {
            handlers::update_credential(state, id, credential_id, request, key_id)?
        }
        ("DELETE", ["profiles", id, "credentials", credential_id]) => {
            handlers::delete_credential(state, id, credential_id, request, key_id)?
        }
        _ => return Err(RouteError::NotFound),
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, path: &str, key: &str, body: &str, extra: &str) -> Request {
        let raw = format!(
            "{method} {path} HTTP/1.1\r\nAuthorization: Bearer {key}\r\nContent-Length: {}\r\n{extra}\r\n{body}",
            body.len()
        );
        super::super::http::parse_request(raw.as_bytes()).unwrap()
    }

    fn body(response: Response) -> serde_json::Value {
        let bytes = response.into_bytes();
        let split = bytes
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap();
        serde_json::from_slice(&bytes[split + 4..]).unwrap()
    }

    #[test]
    fn http_routes_issue_and_consume_a_sensitive_token() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::open(dir.path().to_path_buf()).unwrap();
        let key = state
            .local_api()
            .unwrap()
            .generate_key("test".into())
            .unwrap()
            .full_key;
        let issue = request("POST", "/v1/sensitive-tokens", &key, "{}", "");
        let issued = body(dispatch(&state, &issue));
        assert_eq!(issued["success"], true);
        let token = issued["data"]["token"].as_str().unwrap();
        let extra = format!("X-Veil-Sensitive-Token: {token}\r\n");
        let read = request("GET", "/v1/profiles/missing/cookies", &key, "", &extra);
        let first = body(dispatch(&state, &read));
        assert_eq!(first["error"]["code"], "profile.notFound");
        let second = body(dispatch(&state, &read));
        assert_eq!(second["error"]["code"], "security.verificationFailed");
    }

    #[test]
    fn http_routes_reject_implicit_store_network_access() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::open(dir.path().to_path_buf()).unwrap();
        let key = state
            .local_api()
            .unwrap()
            .generate_key("test".into())
            .unwrap()
            .full_key;
        let request = request(
            "POST",
            "/v1/extensions",
            &key,
            r#"{"storeUrl":"https://chromewebstore.google.com/detail/foo"}"#,
            "",
        );
        let result = body(dispatch(&state, &request));
        assert_eq!(result["error"]["code"], "localApi.badRequest");
    }

    #[test]
    fn formerly_pending_routes_reach_their_handlers() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::open(dir.path().to_path_buf()).unwrap();
        let key = state
            .local_api()
            .unwrap()
            .generate_key("test".into())
            .unwrap()
            .full_key;
        let cases = [
            (
                "POST",
                "/v1/profiles/missing/launch",
                "",
                "launch.profileMissing",
            ),
            (
                "POST",
                "/v1/temp-profiles/launch",
                r#"{"profileId":"missing"}"#,
                "profile.notFound",
            ),
            (
                "POST",
                "/v1/temp-profiles/custom-launch",
                "{}",
                "launch.runtimeMissing",
            ),
            (
                "DELETE",
                "/v1/profiles/missing/data",
                "",
                "profile.notFound",
            ),
            (
                "DELETE",
                "/v1/profiles/missing/cache",
                "",
                "profile.notFound",
            ),
            ("DELETE", "/v1/extensions/missing", "", "extension.notFound"),
        ];
        for (method, path, payload, expected) in cases {
            let request = request(method, path, &key, payload, "");
            let result = body(dispatch(&state, &request));
            assert_eq!(result["error"]["code"], expected, "{path}");
        }
        let close = request("POST", "/v1/temp-profiles/missing/close", &key, "", "");
        let result = body(dispatch(&state, &close));
        assert_eq!(result["data"]["closed"], false);
    }
}
