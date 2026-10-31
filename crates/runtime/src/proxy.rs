//! Proxy connectivity test.
//!
//! Network-touching, but ONLY runs when the user clicks "Test" in the Proxy UI — the
//! runtime layer is the only Veil code allowed to talk to the network. All requests are sent
//! through the user's proxy and target
//! free, HTTPS, no-token endpoints:
//!
//! - https://api.ipify.org?format=json  → `{ "ip": "..." }`
//! - https://ipapi.co/{ip}/json/        → `{ "country_name": "..." }`
//!
//! A response from ipify means the proxy can reach the public internet; the latency we
//! report is the round-trip to ipify alone (ipapi.co is best-effort metadata).

use std::time::{Duration, Instant};

use serde::Deserialize;
use thiserror::Error;

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const IP_URL: &str = "https://api.ipify.org?format=json";
const GEO_URL_PREFIX: &str = "https://ipapi.co/";

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub kind: String, // "http" | "https" | "socks5"
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    /// Build the proxy URL reqwest expects, percent-encoding the credentials.
    fn proxy_url(&self) -> String {
        // Map both http and https proxy kinds to the "http" scheme so reqwest sends the
        // CONNECT through standard HTTP; "https" as a scheme would imply TLS to the proxy
        // itself which we don't want by default.
        let scheme = if self.kind == "socks5" {
            "socks5"
        } else {
            "http"
        };
        match (&self.username, &self.password) {
            (Some(u), Some(p)) if !u.is_empty() => {
                format!(
                    "{scheme}://{}:{}@{host}:{port}",
                    percent_encode(u),
                    percent_encode(p),
                    host = self.host,
                    port = self.port,
                )
            }
            (Some(u), _) if !u.is_empty() => {
                format!(
                    "{scheme}://{}@{host}:{port}",
                    percent_encode(u),
                    host = self.host,
                    port = self.port,
                )
            }
            _ => format!(
                "{scheme}://{host}:{port}",
                host = self.host,
                port = self.port
            ),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProxyTestOutcome {
    pub ip: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Error)]
pub enum ProxyTestError {
    #[error("could not connect through proxy")]
    Connect,
    #[error("proxy connection timed out")]
    Timeout,
    #[error("proxy response could not be parsed")]
    Response,
}

#[derive(Debug, Deserialize)]
struct IpifyResponse {
    ip: String,
}

#[derive(Debug, Deserialize)]
struct IpapiResponse {
    country_name: Option<String>,
    country_code: Option<String>,
    region: Option<String>,
    city: Option<String>,
    timezone: Option<String>,
}

/// Issue an ipify probe through `cfg` and, on success, fetch a country tag from ipapi.co.
/// Blocking — call from a worker thread (e.g. `tauri::async_runtime::spawn_blocking`).
pub fn test_proxy(cfg: &ProxyConfig) -> Result<ProxyTestOutcome, ProxyTestError> {
    let proxy_url = cfg.proxy_url();
    let proxy = reqwest::Proxy::all(&proxy_url).map_err(|_| ProxyTestError::Connect)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .user_agent("veil-proxy-test/0")
        .proxy(proxy)
        .build()
        .map_err(|_| ProxyTestError::Connect)?;

    let start = Instant::now();
    let ip_response = client.get(IP_URL).send().map_err(classify_error)?;
    let ip_response = ip_response
        .error_for_status()
        .map_err(|_| ProxyTestError::Connect)?;
    let ipify: IpifyResponse = ip_response.json().map_err(|_| ProxyTestError::Response)?;
    let latency_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;

    // ipapi is best-effort; failure here doesn't fail the whole test.
    let geo = client
        .get(format!("{GEO_URL_PREFIX}{}/json/", ipify.ip))
        .send()
        .ok()
        .and_then(|r| r.error_for_status().ok())
        .and_then(|r| r.json::<IpapiResponse>().ok());

    Ok(ProxyTestOutcome {
        ip: Some(ipify.ip),
        country: geo.as_ref().and_then(|j| j.country_name.clone()),
        country_code: geo
            .as_ref()
            .and_then(|j| j.country_code.clone())
            .map(|code| code.to_ascii_uppercase()),
        region: geo.as_ref().and_then(|j| j.region.clone()),
        city: geo.as_ref().and_then(|j| j.city.clone()),
        timezone: geo.as_ref().and_then(|j| j.timezone.clone()),
        latency_ms: Some(latency_ms),
    })
}

fn classify_error(err: reqwest::Error) -> ProxyTestError {
    if err.is_timeout() {
        ProxyTestError::Timeout
    } else {
        ProxyTestError::Connect
    }
}

/// Percent-encode the very small set of characters that would otherwise break a URL
/// authority. We don't want to pull in `percent-encoding` for this, and the typical
/// proxy credentials use letters/digits anyway.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            // RFC 3986 unreserved + a couple of common safe characters.
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' => out.push(c),
            other => {
                let mut buf = [0u8; 4];
                for byte in other.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_url_without_creds() {
        let cfg = ProxyConfig {
            kind: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        assert_eq!(cfg.proxy_url(), "http://127.0.0.1:8080");
    }

    #[test]
    fn proxy_url_with_creds_and_percent_encoding() {
        let cfg = ProxyConfig {
            kind: "socks5".to_string(),
            host: "host.example".to_string(),
            port: 1080,
            username: Some("user@1".to_string()),
            password: Some("p ss/word".to_string()),
        };
        // '@', '/', and ' ' must be percent-encoded so they don't break the URL authority.
        assert_eq!(
            cfg.proxy_url(),
            "socks5://user%40\
             1:p%20ss%2Fword@host.example:1080"
                .replace("\\\n             ", "")
        );
    }

    #[test]
    fn test_proxy_against_unbound_port_fails_fast() {
        // 127.0.0.1:1 — almost nothing listens here; both timeout and connect-refused
        // paths are acceptable failures.
        let cfg = ProxyConfig {
            kind: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1,
            username: None,
            password: None,
        };
        let err = test_proxy(&cfg).unwrap_err();
        assert!(matches!(
            err,
            ProxyTestError::Connect | ProxyTestError::Timeout
        ));
    }
}
