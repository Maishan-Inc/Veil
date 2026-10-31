//! DPAPI-sealed database keystore.
//!
//! Holds the SQLCipher database key in a small file under the app's local data directory.
//! The file contents are sealed with Windows DPAPI (current-user scope) so they can only be
//! decrypted by the same Windows account. The unsealed key lives in memory long enough to
//! issue the SQLCipher `PRAGMA key`, then is dropped.
//!
//! Used at first run: if the file is missing we generate a 32-byte random key, seal it, and
//! write it atomically. On subsequent runs we read + unseal.
//!
//! Until the user opts into a master password (Phase 7), this is the at-rest protection
//! for the database key.

use std::fs;
use std::path::{Path, PathBuf};

use rand::rngs::OsRng;
use rand::RngCore;
use thiserror::Error;

#[cfg(windows)]
use crate::dpapi::{protect_bytes, unprotect_bytes, DpapiError};

/// File name of the sealed key blob inside the app's data directory.
pub const KEYFILE_NAME: &str = "veil-key.bin";

/// Raw key length (matches SQLCipher's 256-bit AES key).
pub const RAW_KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("could not read keyfile")]
    Read,
    #[error("could not write keyfile")]
    Write,
    #[error("could not seal keyfile (DPAPI)")]
    Seal,
    #[error("could not open keyfile (DPAPI)")]
    Unseal,
    #[error("keyfile is corrupted")]
    Corrupted,
}

#[cfg(windows)]
impl From<DpapiError> for KeyError {
    fn from(err: DpapiError) -> Self {
        match err {
            DpapiError::Protect => KeyError::Seal,
            DpapiError::Unprotect => KeyError::Unseal,
        }
    }
}

/// Owns the path to the sealed key blob and produces a SQLCipher key on demand.
///
/// The keystore does NOT cache the key; callers are expected to call `load_or_create`
/// once at startup, pass the result to `Database::open`, and drop it.
pub struct DbKeystore {
    keyfile: PathBuf,
}

impl DbKeystore {
    /// Construct a keystore that lives under `data_dir`. The directory is created lazily
    /// the first time `load_or_create` writes a new key.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            keyfile: data_dir.join(KEYFILE_NAME),
        }
    }

    /// Path to the sealed key blob. Useful for diagnostics / logs (the file is sealed).
    pub fn keyfile_path(&self) -> &Path {
        &self.keyfile
    }

    /// Return the SQLCipher PRAGMA key (lowercase hex of 32 random bytes), creating and
    /// sealing a new one if the keyfile does not yet exist.
    pub fn load_or_create(&self) -> Result<String, KeyError> {
        if self.keyfile.exists() {
            return self.load();
        }
        self.create()
    }

    fn load(&self) -> Result<String, KeyError> {
        let sealed = fs::read(&self.keyfile).map_err(|_| KeyError::Read)?;
        let raw = unseal(&sealed)?;
        if raw.len() != RAW_KEY_LEN {
            return Err(KeyError::Corrupted);
        }
        Ok(hex::encode(&raw))
    }

    fn create(&self) -> Result<String, KeyError> {
        if let Some(parent) = self.keyfile.parent() {
            fs::create_dir_all(parent).map_err(|_| KeyError::Write)?;
        }
        let mut raw = [0u8; RAW_KEY_LEN];
        OsRng.fill_bytes(&mut raw);
        let sealed = seal(&raw)?;
        write_atomic(&self.keyfile, &sealed)?;
        Ok(hex::encode(raw))
    }
}

#[cfg(windows)]
fn seal(raw: &[u8]) -> Result<Vec<u8>, KeyError> {
    Ok(protect_bytes(raw)?)
}

#[cfg(windows)]
fn unseal(sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
    Ok(unprotect_bytes(sealed)?)
}

// On non-Windows targets DPAPI is unavailable. To keep tests runnable on CI containers
// (and to keep the crate cross-compilable), we fall back to identity sealing. The
// production target is Windows only, where the cfg(windows) path always wins.
#[cfg(not(windows))]
fn seal(raw: &[u8]) -> Result<Vec<u8>, KeyError> {
    Ok(raw.to_vec())
}

#[cfg(not(windows))]
fn unseal(sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
    Ok(sealed.to_vec())
}

/// Atomic write: temp file in the same directory + rename. Survives a crash mid-write.
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), KeyError> {
    let parent = target.parent().ok_or(KeyError::Write)?;
    let tmp = parent.join(format!(
        "{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("veil-key")
    ));
    fs::write(&tmp, bytes).map_err(|_| KeyError::Write)?;
    fs::rename(&tmp, target).map_err(|_| KeyError::Write)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_then_reloads_same_key() {
        let dir = tempdir().unwrap();
        let store = DbKeystore::new(dir.path());
        let first = store.load_or_create().unwrap();
        assert_eq!(first.len(), RAW_KEY_LEN * 2, "hex of 32 bytes");
        let second = store.load_or_create().unwrap();
        assert_eq!(first, second, "key is stable across reloads");
        assert!(store.keyfile_path().exists());
    }

    #[test]
    fn keyfile_is_sealed_on_windows() {
        let dir = tempdir().unwrap();
        let store = DbKeystore::new(dir.path());
        let hex_key = store.load_or_create().unwrap();
        let raw = hex::decode(&hex_key).unwrap();
        let on_disk = fs::read(store.keyfile_path()).unwrap();
        // On Windows DPAPI must transform the bytes; on other targets the fallback is identity
        // so this assertion is gated.
        #[cfg(windows)]
        assert_ne!(on_disk, raw, "DPAPI must alter the bytes on disk");
        // Either way, the on-disk blob must NOT be a plaintext zero buffer.
        assert_ne!(on_disk.as_slice(), [0u8; RAW_KEY_LEN].as_slice());
        // Drop unused-variable warning on non-Windows.
        let _ = raw;
    }

    #[test]
    fn rejects_corrupted_keyfile() {
        let dir = tempdir().unwrap();
        let store = DbKeystore::new(dir.path());
        // Write garbage that is too small to be a valid sealed blob.
        fs::write(store.keyfile_path(), b"not-a-real-sealed-blob").unwrap();
        let err = store.load_or_create().unwrap_err();
        // On Windows this is Unseal (DPAPI rejects it); on other targets identity yields
        // Corrupted because the length is wrong.
        assert!(
            matches!(err, KeyError::Unseal | KeyError::Corrupted),
            "got {err:?}"
        );
    }
}
