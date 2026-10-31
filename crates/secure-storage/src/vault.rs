//! Versioned SQLCipher DEK wrapping.
//!
//! `DbKeystore` is the legacy DPAPI-only keyfile. `VaultKeystore` introduces a
//! versioned metadata file that can wrap the same random SQLCipher DEK with either
//! Windows DPAPI or a password-derived KEK. The database key remains a random DEK;
//! changing protection mode rewrites wrappers, not the encrypted database.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

use crate::keystore::{DbKeystore, KeyError, RAW_KEY_LEN};
use crate::{derive_key, generate_salt};

#[cfg(windows)]
use crate::{protect_bytes, unprotect_bytes, DpapiError};

pub const VAULT_FILE_NAME: &str = "veil-vault.json";
const VAULT_VERSION: u32 = 1;
const NONCE_LEN: usize = 12;
const RECOVERY_PART_COUNT: usize = 12;
const RECOVERY_PART_LEN: usize = 4;
const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

#[derive(Debug, Error)]
pub enum VaultKeyError {
    #[error("could not read vault metadata")]
    Read,
    #[error("could not write vault metadata")]
    Write,
    #[error("could not seal key material")]
    Seal,
    #[error("could not unseal key material")]
    Unseal,
    #[error("vault metadata is corrupted")]
    Corrupted,
    #[error("password is required to unlock the vault")]
    PasswordRequired,
    #[error("password verification failed")]
    VerificationFailed,
}

impl From<KeyError> for VaultKeyError {
    fn from(err: KeyError) -> Self {
        match err {
            KeyError::Read => VaultKeyError::Read,
            KeyError::Write => VaultKeyError::Write,
            KeyError::Seal => VaultKeyError::Seal,
            KeyError::Unseal => VaultKeyError::Unseal,
            KeyError::Corrupted => VaultKeyError::Corrupted,
        }
    }
}

#[cfg(windows)]
impl From<DpapiError> for VaultKeyError {
    fn from(err: DpapiError) -> Self {
        match err {
            DpapiError::Protect => VaultKeyError::Seal,
            DpapiError::Unprotect => VaultKeyError::Unseal,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VaultWrapperKind {
    Dpapi,
    Password,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultMetadata {
    version: u32,
    active_wrapper: VaultWrapperKind,
    dpapi: Option<DpapiWrapper>,
    password: Option<PasswordWrapper>,
    #[serde(default)]
    recovery: Option<PasswordWrapper>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DpapiWrapper {
    sealed_dek_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasswordWrapper {
    salt_hex: String,
    nonce_hex: String,
    wrapped_dek_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WrappedSecret {
    pub nonce_hex: String,
    pub ciphertext_hex: String,
}

pub fn generate_recovery_phrase() -> Vec<String> {
    let mut parts = Vec::with_capacity(RECOVERY_PART_COUNT);
    let mut seen = HashSet::with_capacity(RECOVERY_PART_COUNT);
    while parts.len() < RECOVERY_PART_COUNT {
        let part = random_recovery_part();
        if seen.insert(part.clone()) {
            parts.push(part);
        }
    }
    parts
}

/// Versioned keystore for the SQLCipher DEK.
///
/// This type does not cache plaintext key material. Each unlock returns the
/// SQLCipher key as lowercase hex, matching `Database::open`.
pub struct VaultKeystore {
    vault_file: PathBuf,
    legacy: DbKeystore,
}

impl VaultKeystore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            vault_file: data_dir.join(VAULT_FILE_NAME),
            legacy: DbKeystore::new(data_dir),
        }
    }

    pub fn vault_file_path(&self) -> &Path {
        &self.vault_file
    }

    /// Open the active wrapper. DPAPI mode can unlock without user input;
    /// password mode returns `PasswordRequired`.
    pub fn load_or_create_dpapi(&self) -> Result<String, VaultKeyError> {
        if self.vault_file.exists() {
            let metadata = self.load_metadata()?;
            return match metadata.active_wrapper {
                VaultWrapperKind::Dpapi => {
                    let wrapper = metadata.dpapi.ok_or(VaultKeyError::Corrupted)?;
                    let mut raw = open_dpapi_wrapper(&wrapper)?;
                    let key = hex::encode(&raw);
                    raw.zeroize();
                    Ok(key)
                }
                VaultWrapperKind::Password => Err(VaultKeyError::PasswordRequired),
            };
        }

        let mut raw = if self.legacy.keyfile_path().exists() {
            decode_raw_key(&self.legacy.load_or_create()?)?
        } else {
            random_raw_key()
        };
        let metadata = VaultMetadata {
            version: VAULT_VERSION,
            active_wrapper: VaultWrapperKind::Dpapi,
            dpapi: Some(create_dpapi_wrapper(&raw)?),
            password: None,
            recovery: None,
        };
        self.save_metadata(&metadata)?;
        let key = hex::encode(&raw);
        raw.zeroize();
        Ok(key)
    }

    /// Replace DPAPI unlock with password unlock while preserving the existing DEK.
    ///
    /// This is the migration primitive for standard encryption mode. Once saved,
    /// the metadata no longer contains a DPAPI wrapper, so the database cannot be
    /// opened without the password-derived KEK.
    pub fn protect_with_password_from_dpapi(&self, password: &[u8]) -> Result<(), VaultKeyError> {
        if password.is_empty() {
            return Err(VaultKeyError::VerificationFailed);
        }
        let mut raw = decode_raw_key(&self.load_or_create_dpapi()?)?;
        let password_wrapper = create_password_wrapper(&raw, password)?;
        let recovery = self
            .load_metadata()
            .ok()
            .and_then(|metadata| metadata.recovery);
        let metadata = VaultMetadata {
            version: VAULT_VERSION,
            active_wrapper: VaultWrapperKind::Password,
            dpapi: None,
            password: Some(password_wrapper),
            recovery,
        };
        self.save_metadata(&metadata)?;
        raw.zeroize();
        Ok(())
    }

    pub fn unlock_with_password(&self, password: &[u8]) -> Result<String, VaultKeyError> {
        let metadata = self.load_metadata()?;
        if metadata.version != VAULT_VERSION {
            return Err(VaultKeyError::Corrupted);
        }
        let wrapper = metadata.password.ok_or(VaultKeyError::PasswordRequired)?;
        let mut raw = open_password_wrapper(&wrapper, password)?;
        let key = hex::encode(&raw);
        raw.zeroize();
        Ok(key)
    }

    pub fn protect_with_recovery_phrase_from_dpapi(
        &self,
        phrase: &[String],
    ) -> Result<(), VaultKeyError> {
        let mut metadata = if self.vault_file.exists() {
            self.load_metadata()?
        } else {
            VaultMetadata {
                version: VAULT_VERSION,
                active_wrapper: VaultWrapperKind::Dpapi,
                dpapi: None,
                password: None,
                recovery: None,
            }
        };
        let mut raw = match metadata.active_wrapper {
            VaultWrapperKind::Dpapi => {
                if let Some(wrapper) = metadata.dpapi.as_ref() {
                    open_dpapi_wrapper(wrapper)?
                } else {
                    let key = self.load_or_create_dpapi()?;
                    metadata = self.load_metadata()?;
                    decode_raw_key(&key)?
                }
            }
            VaultWrapperKind::Password => return Err(VaultKeyError::PasswordRequired),
        };
        metadata.recovery = Some(create_password_wrapper(
            &raw,
            recovery_secret(phrase).as_bytes(),
        )?);
        self.save_metadata(&metadata)?;
        raw.zeroize();
        Ok(())
    }

    pub fn clear_recovery_phrase(&self) -> Result<(), VaultKeyError> {
        if !self.vault_file.exists() {
            return Ok(());
        }
        let mut metadata = self.load_metadata()?;
        metadata.recovery = None;
        self.save_metadata(&metadata)
    }

    pub fn recover_with_recovery_phrase(
        &self,
        phrase: &[String],
        new_password: Option<&[u8]>,
    ) -> Result<String, VaultKeyError> {
        let metadata = self.load_metadata()?;
        let recovery = metadata.recovery.ok_or(VaultKeyError::PasswordRequired)?;
        let mut raw = open_password_wrapper(&recovery, recovery_secret(phrase).as_bytes())?;
        let refreshed_recovery = create_password_wrapper(&raw, recovery_secret(phrase).as_bytes())?;
        let metadata = if let Some(password) = new_password {
            if password.is_empty() {
                raw.zeroize();
                return Err(VaultKeyError::VerificationFailed);
            }
            VaultMetadata {
                version: VAULT_VERSION,
                active_wrapper: VaultWrapperKind::Password,
                dpapi: None,
                password: Some(create_password_wrapper(&raw, password)?),
                recovery: Some(refreshed_recovery),
            }
        } else {
            VaultMetadata {
                version: VAULT_VERSION,
                active_wrapper: VaultWrapperKind::Dpapi,
                dpapi: Some(create_dpapi_wrapper(&raw)?),
                password: None,
                recovery: Some(refreshed_recovery),
            }
        };
        self.save_metadata(&metadata)?;
        let key = hex::encode(&raw);
        raw.zeroize();
        Ok(key)
    }

    pub fn active_wrapper(&self) -> Result<VaultWrapperKind, VaultKeyError> {
        if !self.vault_file.exists() {
            return Ok(VaultWrapperKind::Dpapi);
        }
        Ok(self.load_metadata()?.active_wrapper)
    }

    fn load_metadata(&self) -> Result<VaultMetadata, VaultKeyError> {
        let bytes = fs::read(&self.vault_file).map_err(|_| VaultKeyError::Read)?;
        let metadata: VaultMetadata =
            serde_json::from_slice(&bytes).map_err(|_| VaultKeyError::Corrupted)?;
        if metadata.version != VAULT_VERSION {
            return Err(VaultKeyError::Corrupted);
        }
        Ok(metadata)
    }

    fn save_metadata(&self, metadata: &VaultMetadata) -> Result<(), VaultKeyError> {
        if let Some(parent) = self.vault_file.parent() {
            fs::create_dir_all(parent).map_err(|_| VaultKeyError::Write)?;
        }
        let bytes = serde_json::to_vec_pretty(metadata).map_err(|_| VaultKeyError::Write)?;
        write_atomic(&self.vault_file, &bytes)
    }
}

fn create_dpapi_wrapper(raw: &[u8]) -> Result<DpapiWrapper, VaultKeyError> {
    Ok(DpapiWrapper {
        sealed_dek_hex: hex::encode(seal_dpapi(raw)?),
    })
}

fn open_dpapi_wrapper(wrapper: &DpapiWrapper) -> Result<Vec<u8>, VaultKeyError> {
    let sealed = hex::decode(&wrapper.sealed_dek_hex).map_err(|_| VaultKeyError::Corrupted)?;
    let raw = unseal_dpapi(&sealed)?;
    if raw.len() != RAW_KEY_LEN {
        return Err(VaultKeyError::Corrupted);
    }
    Ok(raw)
}

fn create_password_wrapper(raw: &[u8], password: &[u8]) -> Result<PasswordWrapper, VaultKeyError> {
    let salt = generate_salt();
    let mut kek = derive_key(password, &salt).map_err(|_| VaultKeyError::Seal)?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher = Aes256Gcm::new_from_slice(&kek).map_err(|_| VaultKeyError::Seal)?;
    let wrapped = cipher
        .encrypt(Nonce::from_slice(&nonce), raw)
        .map_err(|_| VaultKeyError::Seal)?;
    kek.zeroize();
    Ok(PasswordWrapper {
        salt_hex: hex::encode(salt),
        nonce_hex: hex::encode(nonce),
        wrapped_dek_hex: hex::encode(wrapped),
    })
}

fn open_password_wrapper(
    wrapper: &PasswordWrapper,
    password: &[u8],
) -> Result<Vec<u8>, VaultKeyError> {
    let salt = hex::decode(&wrapper.salt_hex).map_err(|_| VaultKeyError::Corrupted)?;
    let nonce = hex::decode(&wrapper.nonce_hex).map_err(|_| VaultKeyError::Corrupted)?;
    let wrapped = hex::decode(&wrapper.wrapped_dek_hex).map_err(|_| VaultKeyError::Corrupted)?;
    if nonce.len() != NONCE_LEN {
        return Err(VaultKeyError::Corrupted);
    }
    let mut kek = derive_key(password, &salt).map_err(|_| VaultKeyError::VerificationFailed)?;
    let cipher = Aes256Gcm::new_from_slice(&kek).map_err(|_| VaultKeyError::VerificationFailed)?;
    let raw = cipher
        .decrypt(Nonce::from_slice(&nonce), wrapped.as_ref())
        .map_err(|_| VaultKeyError::VerificationFailed)?;
    kek.zeroize();
    if raw.len() != RAW_KEY_LEN {
        return Err(VaultKeyError::Corrupted);
    }
    Ok(raw)
}

fn recovery_secret(phrase: &[String]) -> String {
    phrase
        .iter()
        .map(|part| part.replace([' ', '-'], "").trim().to_uppercase())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn random_recovery_part() -> String {
    let mut bytes = [0u8; RECOVERY_PART_LEN];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|byte| RECOVERY_ALPHABET[(byte & 31) as usize] as char)
        .collect()
}

pub fn wrap_secret_with_dek_hex(
    dek_hex: &str,
    plaintext: &[u8],
) -> Result<WrappedSecret, VaultKeyError> {
    let mut dek = decode_raw_key(dek_hex)?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher = Aes256Gcm::new_from_slice(&dek).map_err(|_| VaultKeyError::Seal)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| VaultKeyError::Seal)?;
    dek.zeroize();
    Ok(WrappedSecret {
        nonce_hex: hex::encode(nonce),
        ciphertext_hex: hex::encode(ciphertext),
    })
}

pub fn unwrap_secret_with_dek_hex(
    dek_hex: &str,
    wrapped: &WrappedSecret,
) -> Result<Vec<u8>, VaultKeyError> {
    let mut dek = decode_raw_key(dek_hex)?;
    let nonce = hex::decode(&wrapped.nonce_hex).map_err(|_| VaultKeyError::Corrupted)?;
    let ciphertext = hex::decode(&wrapped.ciphertext_hex).map_err(|_| VaultKeyError::Corrupted)?;
    if nonce.len() != NONCE_LEN {
        return Err(VaultKeyError::Corrupted);
    }
    let cipher = Aes256Gcm::new_from_slice(&dek).map_err(|_| VaultKeyError::Unseal)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| VaultKeyError::Unseal)?;
    dek.zeroize();
    Ok(plaintext)
}

fn decode_raw_key(key_hex: &str) -> Result<Vec<u8>, VaultKeyError> {
    let raw = hex::decode(key_hex).map_err(|_| VaultKeyError::Corrupted)?;
    if raw.len() != RAW_KEY_LEN {
        return Err(VaultKeyError::Corrupted);
    }
    Ok(raw)
}

fn random_raw_key() -> Vec<u8> {
    let mut raw = vec![0u8; RAW_KEY_LEN];
    OsRng.fill_bytes(&mut raw);
    raw
}

#[cfg(windows)]
fn seal_dpapi(raw: &[u8]) -> Result<Vec<u8>, VaultKeyError> {
    Ok(protect_bytes(raw)?)
}

#[cfg(windows)]
fn unseal_dpapi(sealed: &[u8]) -> Result<Vec<u8>, VaultKeyError> {
    Ok(unprotect_bytes(sealed)?)
}

#[cfg(not(windows))]
fn seal_dpapi(raw: &[u8]) -> Result<Vec<u8>, VaultKeyError> {
    Ok(raw.to_vec())
}

#[cfg(not(windows))]
fn unseal_dpapi(sealed: &[u8]) -> Result<Vec<u8>, VaultKeyError> {
    Ok(sealed.to_vec())
}

fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), VaultKeyError> {
    let parent = target.parent().ok_or(VaultKeyError::Write)?;
    let tmp = parent.join(format!(
        "{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("veil-vault")
    ));
    fs::write(&tmp, bytes).map_err(|_| VaultKeyError::Write)?;
    fs::rename(&tmp, target).map_err(|_| VaultKeyError::Write)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_dpapi_vault_without_legacy_keyfile() {
        let dir = tempdir().unwrap();
        let vault = VaultKeystore::new(dir.path());
        let key = vault.load_or_create_dpapi().unwrap();
        assert_eq!(key.len(), RAW_KEY_LEN * 2);
        assert!(vault.vault_file_path().exists());
        assert!(!DbKeystore::new(dir.path()).keyfile_path().exists());
        assert_eq!(vault.active_wrapper().unwrap(), VaultWrapperKind::Dpapi);
    }

    #[test]
    fn migrates_existing_legacy_dpapi_key() {
        let dir = tempdir().unwrap();
        let legacy = DbKeystore::new(dir.path());
        let legacy_key = legacy.load_or_create().unwrap();

        let vault = VaultKeystore::new(dir.path());
        let migrated_key = vault.load_or_create_dpapi().unwrap();
        assert_eq!(migrated_key, legacy_key);
        assert!(vault.vault_file_path().exists());
    }

    #[test]
    fn password_wrapper_unlocks_same_dek_and_rejects_wrong_password() {
        let dir = tempdir().unwrap();
        let vault = VaultKeystore::new(dir.path());
        let dpapi_key = vault.load_or_create_dpapi().unwrap();
        vault
            .protect_with_password_from_dpapi(b"correct horse battery staple")
            .unwrap();

        assert_eq!(vault.active_wrapper().unwrap(), VaultWrapperKind::Password);
        assert!(matches!(
            vault.load_or_create_dpapi(),
            Err(VaultKeyError::PasswordRequired)
        ));
        assert_eq!(
            vault
                .unlock_with_password(b"correct horse battery staple")
                .unwrap(),
            dpapi_key
        );
        assert!(matches!(
            vault.unlock_with_password(b"wrong password"),
            Err(VaultKeyError::VerificationFailed)
        ));
    }

    #[test]
    fn password_vault_does_not_store_plaintext_dek_hex() {
        let dir = tempdir().unwrap();
        let vault = VaultKeystore::new(dir.path());
        let key = vault.load_or_create_dpapi().unwrap();
        vault
            .protect_with_password_from_dpapi(b"correct horse battery staple")
            .unwrap();

        let raw_metadata = fs::read_to_string(vault.vault_file_path()).unwrap();
        assert!(!raw_metadata.contains(&key));
        assert!(!raw_metadata.contains("correct horse"));
    }

    #[test]
    fn generated_recovery_phrase_has_twelve_unique_parts() {
        let phrase = generate_recovery_phrase();
        let unique = phrase.iter().collect::<HashSet<_>>();
        assert_eq!(phrase.len(), 12);
        assert_eq!(unique.len(), 12);
        assert!(phrase.iter().all(|part| part.len() == 4));
        assert!(phrase.iter().all(|part| part
            .chars()
            .all(|ch| { ch.is_ascii_uppercase() || matches!(ch, '2'..='7') })));
    }

    #[test]
    fn recovery_phrase_can_create_missing_dpapi_vault() {
        let dir = tempdir().unwrap();
        let phrase = generate_recovery_phrase();
        let vault = VaultKeystore::new(dir.path());
        vault
            .protect_with_recovery_phrase_from_dpapi(&phrase)
            .unwrap();
        assert_eq!(vault.active_wrapper().unwrap(), VaultWrapperKind::Dpapi);
        let dpapi_key = vault.load_or_create_dpapi().unwrap();
        assert_eq!(
            vault.recover_with_recovery_phrase(&phrase, None).unwrap(),
            dpapi_key
        );
    }

    #[test]
    fn recovery_phrase_recovers_dek_and_resets_password_wrapper() {
        let dir = tempdir().unwrap();
        let phrase = vec![
            "AAAA".to_string(),
            "BBBB".to_string(),
            "CCCC".to_string(),
            "DDDD".to_string(),
            "EEEE".to_string(),
            "FFFF".to_string(),
            "GGGG".to_string(),
            "HHHH".to_string(),
            "IIII".to_string(),
            "JJJJ".to_string(),
            "KKKK".to_string(),
            "LLLL".to_string(),
        ];
        let vault = VaultKeystore::new(dir.path());
        let key = vault.load_or_create_dpapi().unwrap();
        vault
            .protect_with_recovery_phrase_from_dpapi(&phrase)
            .unwrap();
        vault
            .protect_with_password_from_dpapi(b"correct horse battery staple")
            .unwrap();

        assert_eq!(
            vault
                .recover_with_recovery_phrase(&phrase, Some(b"new correct horse"))
                .unwrap(),
            key
        );
        assert_eq!(
            vault.unlock_with_password(b"new correct horse").unwrap(),
            key
        );
        assert!(matches!(
            vault.unlock_with_password(b"correct horse battery staple"),
            Err(VaultKeyError::VerificationFailed)
        ));
        let mut wrong = phrase.clone();
        wrong[0] = "ZZZZ".to_string();
        assert!(matches!(
            vault.recover_with_recovery_phrase(&wrong, Some(b"another password")),
            Err(VaultKeyError::VerificationFailed)
        ));
    }

    #[test]
    fn wraps_small_secret_with_dek() {
        let key = hex::encode([7u8; RAW_KEY_LEN]);
        let wrapped = wrap_secret_with_dek_hex(&key, b"totp-secret").unwrap();
        assert_ne!(wrapped.ciphertext_hex, hex::encode("totp-secret"));
        assert_eq!(
            unwrap_secret_with_dek_hex(&key, &wrapped).unwrap(),
            b"totp-secret"
        );
    }
}
