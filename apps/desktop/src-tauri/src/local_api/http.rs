//! Minimal HTTP/1.x reader/writer for the loopback local API.
//!
//! Veil deliberately carries no HTTP framework. The local API is a loopback automation
//! endpoint, so a hand-rolled reader/writer is enough: one request per connection,
//! `Content-Length` framing, `Connection: close`. Requests are tiny JSON; there is no
//! chunked encoding, keep-alive, or pipelining, and none is needed.

use std::io::{Read, Write};
use std::net::TcpStream;

/// Upper bound on a request line + headers, and on the body. Keeps a malformed or
/// malicious client from pinning the listener thread on an unbounded read.
const MAX_HEAD_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// A parsed HTTP request. Header names are lowercased so lookups are case-insensitive.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// Raw target path without the query string, e.g. `/v1/profiles`.
    pub path: String,
    /// Raw query string without the leading `?` (empty when absent).
    pub query: String,
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    /// URL-decoded, non-empty path segments, e.g. `/v1/profiles/abc` → `["v1", "profiles", "abc"]`.
    pub fn segments(&self) -> Vec<String> {
        self.path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(percent_decode)
            .collect()
    }

    /// First value for a query key, e.g. `?groupId=x` → `Some("x")`.
    pub fn query_param(&self, key: &str) -> Option<&str> {
        self.query.split('&').find_map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (name == key).then_some(value)
        })
    }
}

/// An HTTP response. Bodies are JSON; the envelope shape lives in the router.
pub struct Response {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self {
            status,
            reason: reason_phrase(status),
            content_type: "application/json",
            body: serde_json::to_vec(value).unwrap_or_else(|_| br#"{"success":false}"#.to_vec()),
        }
    }

    /// Serialize to a wire representation. Pure so the tests can assert on the bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status,
            self.reason,
            self.content_type,
            self.body.len(),
        )
        .into_bytes();
        head.extend_from_slice(&self.body);
        head
    }

    pub fn write_to(self, stream: &mut TcpStream) -> std::io::Result<()> {
        stream.write_all(&self.into_bytes())?;
        stream.flush()
    }
}

/// Read one HTTP request off a stream: the head (request line + headers) and exactly
/// `Content-Length` body bytes. Returns [`Request`] parsed from the whole message.
pub fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok();
    let message = read_message(stream)?;
    parse_request(&message)
}

fn read_message(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];

    // Read until the end of the header block.
    while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if buf.len() > MAX_HEAD_BYTES {
            return Err(invalid_data("request head too large"));
        }
    }

    let header_end = buf
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| invalid_data("missing header terminator"))?;
    let content_length = content_length(&buf[..header_end]);

    // Read the remainder of the body, if any.
    while buf.len() < header_end + 4 + content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if buf.len() > header_end + 4 + MAX_BODY_BYTES {
            return Err(invalid_data("request body too large"));
        }
    }
    Ok(buf)
}

/// Parse a fully-read HTTP message (head + body). Pure and unit-testable.
pub(crate) fn parse_request(message: &[u8]) -> std::io::Result<Request> {
    let header_end = message
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| invalid_data("missing header terminator"))?;
    let head = std::str::from_utf8(&message[..header_end])
        .map_err(|_| invalid_data("non-utf8 request head"))?;

    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or_else(|| invalid_data("empty request"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| invalid_data("missing method"))?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| invalid_data("missing target"))?
        .to_string();
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (target, String::new()),
    };

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }

    let content_length = content_length(&message[..header_end]);
    let mut body = message[header_end + 4..].to_vec();
    body.truncate(content_length);

    Ok(Request {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn content_length(head: &[u8]) -> usize {
    let head = match std::str::from_utf8(head) {
        Ok(head) => head,
        Err(_) => return 0,
    };
    head.split("\r\n")
        .skip(1)
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn invalid_data(reason: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, reason)
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
            }
            b'+' => out.push(b' '),
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_get_request_with_query() {
        let raw = b"GET /v1/profiles?groupId=abc&status=idle HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer veil_x\r\n\r\n";
        let request = parse_request(raw).unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/v1/profiles");
        assert_eq!(request.query, "groupId=abc&status=idle");
        assert_eq!(request.query_param("groupId"), Some("abc"));
        assert_eq!(request.query_param("status"), Some("idle"));
        assert_eq!(request.query_param("missing"), None);
        assert_eq!(request.header("authorization"), Some("Bearer veil_x"));
        assert_eq!(request.segments(), vec!["v1", "profiles"]);
        assert!(request.body.is_empty());
    }

    #[test]
    fn parses_a_json_body_by_content_length() {
        let body = br#"{"name":"Storefront US"}"#;
        let mut raw = Vec::new();
        raw.extend_from_slice(
            b"POST /v1/profiles HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: ",
        );
        raw.extend_from_slice(body.len().to_string().as_bytes());
        raw.extend_from_slice(b"\r\n\r\n");
        raw.extend_from_slice(body);
        let request = parse_request(&raw).unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.body, body);
    }

    #[test]
    fn percent_decodes_path_segments() {
        let raw = b"GET /v1/profiles/a%20b%2Fc HTTP/1.1\r\n\r\n";
        let request = parse_request(raw).unwrap();
        assert_eq!(request.segments(), vec!["v1", "profiles", "a b/c"]);
    }

    #[test]
    fn response_serializes_with_content_length_and_close() {
        let response = Response::json(200, &serde_json::json!({"success": true}));
        let bytes = response.into_bytes();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 16\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.ends_with("{\"success\":true}"));
    }
}
