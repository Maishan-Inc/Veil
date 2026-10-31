use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::RngCore;
use sha1::Sha1;
use thiserror::Error;

const SECRET_LEN: usize = 20;
const PERIOD_SECONDS: u64 = 30;
const DIGITS: u32 = 6;
const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum TotpError {
    #[error("invalid TOTP secret")]
    InvalidSecret,
    #[error("invalid TOTP code")]
    InvalidCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpEnrollment {
    pub secret_base32: String,
    pub otpauth_uri: String,
}

pub fn generate_totp_secret() -> Vec<u8> {
    let mut secret = vec![0u8; SECRET_LEN];
    OsRng.fill_bytes(&mut secret);
    secret
}

pub fn create_totp_enrollment(issuer: &str, account: &str) -> TotpEnrollment {
    let secret = generate_totp_secret();
    let secret_base32 = encode_base32_no_padding(&secret);
    TotpEnrollment {
        otpauth_uri: otpauth_uri(issuer, account, &secret_base32),
        secret_base32,
    }
}

pub fn decode_totp_secret(secret_base32: &str) -> Result<Vec<u8>, TotpError> {
    decode_base32_no_padding(secret_base32)
}

pub fn encode_totp_secret(secret: &[u8]) -> String {
    encode_base32_no_padding(secret)
}

pub fn totp_code(secret: &[u8], unix_time_seconds: u64) -> Result<String, TotpError> {
    let counter = unix_time_seconds / PERIOD_SECONDS;
    hotp_code(secret, counter, DIGITS)
}

pub fn verify_totp_code(secret: &[u8], code: &str, unix_time_seconds: u64) -> bool {
    let normalized = code.trim();
    if normalized.len() != DIGITS as usize || !normalized.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }

    let counter = unix_time_seconds / PERIOD_SECONDS;
    for candidate in counter.saturating_sub(1)..=counter + 1 {
        if hotp_code(secret, candidate, DIGITS).as_deref() == Ok(normalized) {
            return true;
        }
    }
    false
}

fn hotp_code(secret: &[u8], counter: u64, digits: u32) -> Result<String, TotpError> {
    if secret.is_empty() {
        return Err(TotpError::InvalidSecret);
    }
    let mut mac = HmacSha1::new_from_slice(secret).map_err(|_| TotpError::InvalidSecret)?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = (((digest[offset] & 0x7f) as u32) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    let modulus = 10u32.pow(digits);
    Ok(format!(
        "{:0width$}",
        binary % modulus,
        width = digits as usize
    ))
}

fn otpauth_uri(issuer: &str, account: &str, secret_base32: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits={}&period={}",
        percent_encode(issuer),
        percent_encode(account),
        secret_base32,
        percent_encode(issuer),
        DIGITS,
        PERIOD_SECONDS,
    )
}

fn encode_base32_no_padding(bytes: &[u8]) -> String {
    let mut output = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer = 0u32;
    let mut bits_left = 0u8;
    for &byte in bytes {
        buffer = (buffer << 8) | byte as u32;
        bits_left += 8;
        while bits_left >= 5 {
            let index = ((buffer >> (bits_left - 5)) & 0x1f) as usize;
            output.push(BASE32_ALPHABET[index] as char);
            bits_left -= 5;
        }
    }
    if bits_left > 0 {
        let index = ((buffer << (5 - bits_left)) & 0x1f) as usize;
        output.push(BASE32_ALPHABET[index] as char);
    }
    output
}

fn decode_base32_no_padding(value: &str) -> Result<Vec<u8>, TotpError> {
    let mut output = Vec::with_capacity(value.len() * 5 / 8);
    let mut buffer = 0u32;
    let mut bits_left = 0u8;

    for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            continue;
        }
        let upper = byte.to_ascii_uppercase();
        let index = match upper {
            b'A'..=b'Z' => upper - b'A',
            b'2'..=b'7' => upper - b'2' + 26,
            _ => return Err(TotpError::InvalidSecret),
        };
        buffer = (buffer << 5) | index as u32;
        bits_left += 5;
        if bits_left >= 8 {
            output.push(((buffer >> (bits_left - 8)) & 0xff) as u8);
            bits_left -= 8;
        }
    }

    if output.is_empty() {
        return Err(TotpError::InvalidSecret);
    }
    Ok(output)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_round_trips_secret() {
        let secret = b"12345678901234567890";
        let encoded = encode_totp_secret(secret);
        assert_eq!(encoded, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(decode_totp_secret(&encoded).unwrap(), secret);
    }

    #[test]
    fn hotp_matches_rfc_4226_vector() {
        let secret = b"12345678901234567890";
        assert_eq!(hotp_code(secret, 0, 6).unwrap(), "755224");
        assert_eq!(hotp_code(secret, 1, 6).unwrap(), "287082");
    }

    #[test]
    fn verifies_current_adjacent_window_only() {
        let secret = b"12345678901234567890";
        let code = totp_code(secret, 60).unwrap();
        assert!(verify_totp_code(secret, &code, 60));
        assert!(verify_totp_code(secret, &code, 89));
        assert!(!verify_totp_code(secret, &code, 150));
        assert!(!verify_totp_code(secret, "123", 60));
    }

    #[test]
    fn creates_otpauth_uri() {
        let enrollment = create_totp_enrollment("Veil", "Local Vault");
        assert!(enrollment
            .otpauth_uri
            .starts_with("otpauth://totp/Veil:Local%20Vault"));
        assert!(enrollment.otpauth_uri.contains("issuer=Veil"));
        assert!(!enrollment.secret_base32.is_empty());
    }
}
