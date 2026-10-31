//! Local secret protection for Veil.
//!
//! - Master-password key derivation with Argon2id (cross-platform).
//! - On Windows, DPAPI sealing of the database key when no master password is set.
//! - DPAPI-sealed keystore for the SQLCipher database key (see [`keystore`]).
//!
//! No plaintext secret is written outside the encrypted database;
//! the derived/sealed key lives only in process memory.

mod key;
pub use key::{derive_key, generate_salt, KdfError, KEY_LEN, SALT_LEN};

#[cfg(windows)]
mod dpapi;
#[cfg(windows)]
pub use dpapi::{protect_bytes, unprotect_bytes, DpapiError};

pub mod keystore;
pub use keystore::{DbKeystore, KeyError, KEYFILE_NAME, RAW_KEY_LEN};

pub mod vault;
pub use vault::{
    generate_recovery_phrase, unwrap_secret_with_dek_hex, wrap_secret_with_dek_hex, VaultKeyError,
    VaultKeystore, VaultWrapperKind, WrappedSecret, VAULT_FILE_NAME,
};

pub mod totp;
pub use totp::{
    create_totp_enrollment, decode_totp_secret, encode_totp_secret, generate_totp_secret,
    totp_code, verify_totp_code, TotpEnrollment, TotpError,
};
