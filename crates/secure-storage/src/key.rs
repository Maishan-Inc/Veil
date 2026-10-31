use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use thiserror::Error;

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;

#[derive(Debug, Error)]
pub enum KdfError {
    #[error("salt is too short")]
    InvalidSalt,
    #[error("invalid Argon2 parameters")]
    Params,
    #[error("key derivation failed")]
    Hash,
}

/// Generate a cryptographically-random salt.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    salt
}

/// Derive a 32-byte key from a password + salt using Argon2id (memory-hard).
///
/// Parameters: 19 MiB memory, 2 iterations, 1 lane — a balanced interactive cost suitable
/// for unlocking a local vault; re-tune these numbers here if the cost needs to change.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN], KdfError> {
    if salt.len() < 8 {
        return Err(KdfError::InvalidSalt);
    }
    let params = Params::new(19 * 1024, 2, 1, Some(KEY_LEN)).map_err(|_| KdfError::Params)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|_| KdfError::Hash)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic_per_salt() {
        let salt = [7u8; SALT_LEN];
        let a = derive_key(b"correct horse battery staple", &salt).unwrap();
        let b = derive_key(b"correct horse battery staple", &salt).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), KEY_LEN);
    }

    #[test]
    fn different_salts_yield_different_keys() {
        let a = derive_key(b"pw", &[1u8; SALT_LEN]).unwrap();
        let b = derive_key(b"pw", &[2u8; SALT_LEN]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn rejects_short_salt() {
        assert!(matches!(
            derive_key(b"pw", &[0u8; 4]),
            Err(KdfError::InvalidSalt)
        ));
    }

    #[test]
    fn salt_has_expected_length() {
        assert_eq!(generate_salt().len(), SALT_LEN);
    }
}
