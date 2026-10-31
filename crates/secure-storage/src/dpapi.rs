//! Windows DPAPI wrapper (current-user scope). Targets the `windows` crate 0.58.
//!
//! Used to seal the randomly-generated database key when the user has not set a master
//! password, binding it to the Windows account.

use thiserror::Error;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

#[derive(Debug, Error)]
pub enum DpapiError {
    #[error("DPAPI protect failed")]
    Protect,
    #[error("DPAPI unprotect failed")]
    Unprotect,
}

fn as_blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    }
}

/// Copy the output blob into an owned Vec, then free the OS-allocated buffer.
///
/// # Safety
/// `blob` must be a valid output blob returned by a successful Crypt(Un)ProtectData call.
unsafe fn take_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let out = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec();
    let _ = LocalFree(HLOCAL(blob.pbData as *mut core::ffi::c_void));
    out
}

/// Encrypt bytes for the current Windows user.
pub fn protect_bytes(plaintext: &[u8]) -> Result<Vec<u8>, DpapiError> {
    let input = as_blob(plaintext);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| DpapiError::Protect)?;
        Ok(take_blob(output))
    }
}

/// Decrypt bytes previously sealed with [`protect_bytes`] by the current user.
pub fn unprotect_bytes(ciphertext: &[u8]) -> Result<Vec<u8>, DpapiError> {
    let input = as_blob(ciphertext);
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| DpapiError::Unprotect)?;
        Ok(take_blob(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_secret_bytes() {
        let secret = b"veil-database-key-0123456789";
        let sealed = protect_bytes(secret).expect("protect");
        assert_ne!(sealed.as_slice(), secret);
        let opened = unprotect_bytes(&sealed).expect("unprotect");
        assert_eq!(opened.as_slice(), secret);
    }
}
