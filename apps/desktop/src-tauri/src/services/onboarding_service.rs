use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use veil_core::{AppError, AppResult, ErrorCode};
use veil_secure_storage::{
    create_totp_enrollment, decode_totp_secret, unwrap_secret_with_dek_hex, verify_totp_code,
    wrap_secret_with_dek_hex, VaultKeyError, VaultKeystore, VaultWrapperKind, WrappedSecret,
};

const CONFIG_FILE: &str = "onboarding.json";
const DEVELOPER: &str = "Maishan Inc.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatus {
    pub completed: bool,
    pub agreement_accepted_at: Option<String>,
    pub encryption_mode: EncryptionMode,
    pub password_required: bool,
    pub two_factor_enabled: bool,
    pub two_factor_pending: bool,
    pub mnemonic_enabled: bool,
    pub developer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompleteOnboardingInput {
    pub encryption_mode: EncryptionMode,
    pub password: Option<String>,
    pub two_factor_enabled: bool,
    pub mnemonic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoverSecurityInput {
    pub encryption_mode: EncryptionMode,
    pub password: Option<String>,
    pub two_factor_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPhraseOutput {
    pub phrase: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EncryptionMode {
    None,
    Standard,
    Advanced,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveActionInput {
    pub password: Option<String>,
    pub two_factor_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TotpEnrollmentOutput {
    pub secret: String,
    pub otpauth_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOnboarding {
    completed: bool,
    agreement_accepted_at: Option<String>,
    encryption_mode: EncryptionMode,
    password_required: bool,
    two_factor_enabled: bool,
    #[serde(default)]
    two_factor_requested: bool,
    mnemonic_enabled: bool,
    developer: String,
    password_salt_hex: Option<String>,
    password_hash_hex: Option<String>,
    totp_secret_nonce_hex: Option<String>,
    totp_secret_ciphertext_hex: Option<String>,
}

#[derive(Clone)]
pub struct OnboardingService {
    data_dir: PathBuf,
    path: PathBuf,
}

impl OnboardingService {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            path: data_dir.join(CONFIG_FILE),
        }
    }

    pub fn status(&self) -> OnboardingStatus {
        self.load().unwrap_or_default().status()
    }

    pub fn complete(&self, input: CompleteOnboardingInput) -> AppResult<OnboardingStatus> {
        Self::validate_complete_input(&input)?;
        let password = input
            .password
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let (password_salt_hex, password_hash_hex) = hash_password_for_storage(password)?;

        let previous = self.load().unwrap_or_default();
        let preserve_totp = input.two_factor_enabled
            && previous.two_factor_enabled
            && previous.wrapped_totp_secret().is_some();

        let stored = StoredOnboarding {
            completed: true,
            agreement_accepted_at: Some(now_rfc3339()),
            encryption_mode: input.encryption_mode,
            password_required: password.is_some()
                || input.encryption_mode == EncryptionMode::Standard,
            two_factor_enabled: preserve_totp,
            two_factor_requested: input.two_factor_enabled,
            mnemonic_enabled: input.mnemonic_enabled,
            developer: DEVELOPER.to_string(),
            password_salt_hex,
            password_hash_hex,
            totp_secret_nonce_hex: preserve_totp
                .then_some(previous.totp_secret_nonce_hex)
                .flatten(),
            totp_secret_ciphertext_hex: preserve_totp
                .then_some(previous.totp_secret_ciphertext_hex)
                .flatten(),
        };
        self.save(&stored)?;
        Ok(stored.status())
    }

    pub fn recover_security(&self, input: RecoverSecurityInput) -> AppResult<OnboardingStatus> {
        Self::validate_recover_security_input(&input)?;
        let password = input
            .password
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (password_salt_hex, password_hash_hex) = hash_password_for_storage(password)?;
        let previous = self.load().unwrap_or_default();
        let stored = StoredOnboarding {
            completed: true,
            agreement_accepted_at: previous
                .agreement_accepted_at
                .or_else(|| Some(now_rfc3339())),
            encryption_mode: input.encryption_mode,
            password_required: password.is_some()
                || input.encryption_mode == EncryptionMode::Standard,
            two_factor_enabled: false,
            two_factor_requested: input.two_factor_enabled,
            mnemonic_enabled: true,
            developer: DEVELOPER.to_string(),
            password_salt_hex,
            password_hash_hex,
            totp_secret_nonce_hex: None,
            totp_secret_ciphertext_hex: None,
        };
        self.save(&stored)?;
        Ok(stored.status())
    }

    pub fn validate_recover_security_input(input: &RecoverSecurityInput) -> AppResult<()> {
        let complete_input = CompleteOnboardingInput {
            encryption_mode: input.encryption_mode,
            password: input.password.clone(),
            two_factor_enabled: input.two_factor_enabled,
            mnemonic_enabled: true,
        };
        Self::validate_complete_input(&complete_input)
    }

    pub fn validate_complete_input(input: &CompleteOnboardingInput) -> AppResult<()> {
        let password = input
            .password
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if input.encryption_mode == EncryptionMode::Advanced {
            return Err(ErrorCode::StorageEncryptionFailed.into());
        }
        if input.encryption_mode == EncryptionMode::Standard && password.is_none() {
            return Err(ErrorCode::StorageEncryptionFailed.into());
        }
        if let Some(value) = password {
            if value.chars().count() < 8 {
                return Err(ErrorCode::StorageEncryptionFailed.into());
            }
        }
        Ok(())
    }

    pub fn reset(&self) -> AppResult<OnboardingStatus> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        }
        Ok(StoredOnboarding::default().status())
    }

    pub fn verify_sensitive_action(&self, input: SensitiveActionInput) -> AppResult<()> {
        let stored = self.load()?;
        if !stored.completed {
            return Ok(());
        }
        self.verify_sensitive_action_for_stored(&stored, input)?;
        Ok(())
    }

    pub fn begin_totp_enrollment(
        &self,
        verification: SensitiveActionInput,
    ) -> AppResult<TotpEnrollmentOutput> {
        let stored = self.load()?;
        if stored.completed {
            self.verify_sensitive_action_for_stored(&stored, verification)?;
        }
        let enrollment = create_totp_enrollment(DEVELOPER, "Veil");
        Ok(TotpEnrollmentOutput {
            secret: enrollment.secret_base32,
            otpauth_uri: enrollment.otpauth_uri,
        })
    }

    pub fn confirm_totp_enrollment(
        &self,
        secret: &str,
        code: &str,
        verification: SensitiveActionInput,
    ) -> AppResult<OnboardingStatus> {
        let mut stored = self.load()?;
        let password = if stored.completed {
            self.verify_sensitive_action_for_stored(&stored, verification)?
        } else {
            None
        };
        let raw_secret = decode_totp_secret(secret)
            .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        if !verify_totp_code(&raw_secret, code, current_unix_time()) {
            return Err(ErrorCode::SecurityVerificationFailed.into());
        }

        let dek = self.unlock_vault_dek(password.as_deref())?;
        let wrapped = wrap_secret_with_dek_hex(&dek, &raw_secret)
            .map_err(|err| AppError::from(map_vault_key_error(err)))?;
        stored.two_factor_enabled = true;
        stored.two_factor_requested = true;
        stored.totp_secret_nonce_hex = Some(wrapped.nonce_hex);
        stored.totp_secret_ciphertext_hex = Some(wrapped.ciphertext_hex);
        self.save(&stored)?;
        Ok(stored.status())
    }

    pub fn disable_totp(&self, verification: SensitiveActionInput) -> AppResult<OnboardingStatus> {
        let mut stored = self.load()?;
        self.verify_sensitive_action_for_stored(&stored, verification)?;
        stored.two_factor_enabled = false;
        stored.totp_secret_nonce_hex = None;
        stored.totp_secret_ciphertext_hex = None;
        self.save(&stored)?;
        Ok(stored.status())
    }

    fn verify_sensitive_action_for_stored(
        &self,
        stored: &StoredOnboarding,
        input: SensitiveActionInput,
    ) -> AppResult<Option<String>> {
        if stored.password_required {
            let password = input
                .password
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
            if !stored.verify_password(password)? {
                return Err(ErrorCode::SecurityVerificationFailed.into());
            }
            if stored.two_factor_enabled {
                self.verify_totp_for_stored(stored, Some(password), input.two_factor_code)?;
            }
            return Ok(Some(password.to_string()));
        }

        if stored.two_factor_enabled {
            self.verify_totp_for_stored(stored, None, input.two_factor_code)?;
        }
        Ok(None)
    }

    fn verify_totp_for_stored(
        &self,
        stored: &StoredOnboarding,
        password: Option<&str>,
        code: Option<String>,
    ) -> AppResult<()> {
        let code = code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let secret = self.decrypt_totp_secret(stored, password)?;
        if verify_totp_code(&secret, code, current_unix_time()) {
            Ok(())
        } else {
            Err(ErrorCode::SecurityVerificationFailed.into())
        }
    }

    fn decrypt_totp_secret(
        &self,
        stored: &StoredOnboarding,
        password: Option<&str>,
    ) -> AppResult<Vec<u8>> {
        let wrapped = stored
            .wrapped_totp_secret()
            .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let dek = self.unlock_vault_dek(password)?;
        unwrap_secret_with_dek_hex(&dek, &wrapped)
            .map_err(|err| AppError::from(map_vault_key_error(err)))
    }

    fn unlock_vault_dek(&self, password: Option<&str>) -> AppResult<String> {
        let vault = VaultKeystore::new(&self.data_dir);
        match vault.active_wrapper().unwrap_or(VaultWrapperKind::Dpapi) {
            VaultWrapperKind::Dpapi => vault
                .load_or_create_dpapi()
                .map_err(|err| AppError::from(map_vault_key_error(err))),
            VaultWrapperKind::Password => {
                let password = password
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
                vault
                    .unlock_with_password(password.as_bytes())
                    .map_err(|err| AppError::from(map_vault_key_error(err)))
            }
        }
    }

    fn load(&self) -> AppResult<StoredOnboarding> {
        if !self.path.exists() {
            return Ok(StoredOnboarding::default());
        }
        let bytes = fs::read(&self.path)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseReadFailed))?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseReadFailed))
    }

    fn save(&self, stored: &StoredOnboarding) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        }
        let bytes = serde_json::to_vec_pretty(stored)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, bytes)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        fs::rename(&tmp, &self.path)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        Ok(())
    }
}

impl StoredOnboarding {
    fn status(&self) -> OnboardingStatus {
        OnboardingStatus {
            completed: self.completed,
            agreement_accepted_at: self.agreement_accepted_at.clone(),
            encryption_mode: self.encryption_mode,
            password_required: self.password_required,
            two_factor_enabled: self.two_factor_enabled && self.wrapped_totp_secret().is_some(),
            two_factor_pending: self.two_factor_requested
                && !(self.two_factor_enabled && self.wrapped_totp_secret().is_some()),
            mnemonic_enabled: self.mnemonic_enabled,
            developer: self.developer.clone(),
        }
    }

    fn verify_password(&self, password: &str) -> AppResult<bool> {
        let salt_hex = self
            .password_salt_hex
            .as_deref()
            .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let hash_hex = self
            .password_hash_hex
            .as_deref()
            .ok_or_else(|| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let salt = hex::decode(salt_hex)
            .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let expected = hex::decode(hash_hex)
            .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        let actual = veil_secure_storage::derive_key(password.as_bytes(), &salt)
            .map_err(|_| AppError::from(ErrorCode::SecurityVerificationFailed))?;
        Ok(constant_time_eq(&actual, &expected))
    }

    fn wrapped_totp_secret(&self) -> Option<WrappedSecret> {
        Some(WrappedSecret {
            nonce_hex: self.totp_secret_nonce_hex.clone()?,
            ciphertext_hex: self.totp_secret_ciphertext_hex.clone()?,
        })
    }
}

impl Default for StoredOnboarding {
    fn default() -> Self {
        Self {
            completed: false,
            agreement_accepted_at: None,
            encryption_mode: EncryptionMode::None,
            password_required: false,
            two_factor_enabled: false,
            two_factor_requested: false,
            mnemonic_enabled: false,
            developer: DEVELOPER.to_string(),
            password_salt_hex: None,
            password_hash_hex: None,
            totp_secret_nonce_hex: None,
            totp_secret_ciphertext_hex: None,
        }
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn hash_password_for_storage(
    password: Option<&str>,
) -> AppResult<(Option<String>, Option<String>)> {
    match password {
        Some(value) => {
            let salt = veil_secure_storage::generate_salt();
            let hash = veil_secure_storage::derive_key(value.as_bytes(), &salt)
                .map_err(|_| AppError::from(ErrorCode::StorageEncryptionFailed))?;
            Ok((Some(hex::encode(salt)), Some(hex::encode(hash))))
        }
        None => Ok((None, None)),
    }
}

fn current_unix_time() -> u64 {
    OffsetDateTime::now_utc().unix_timestamp().max(0) as u64
}

fn map_vault_key_error(err: VaultKeyError) -> ErrorCode {
    match err {
        VaultKeyError::Read
        | VaultKeyError::Unseal
        | VaultKeyError::Corrupted
        | VaultKeyError::PasswordRequired => ErrorCode::StorageDecryptionFailed,
        VaultKeyError::VerificationFailed => ErrorCode::SecurityVerificationFailed,
        VaultKeyError::Write | VaultKeyError::Seal => ErrorCode::StorageEncryptionFailed,
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_defaults_to_incomplete() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        let status = service.status();
        assert!(!status.completed);
        assert_eq!(status.developer, DEVELOPER);
    }

    #[test]
    fn completes_without_password_when_mode_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        let status = service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::None,
                password: None,
                two_factor_enabled: false,
                mnemonic_enabled: false,
            })
            .expect("complete");

        assert!(status.completed);
        assert!(!status.password_required);
        assert!(service.status().completed);
    }

    #[test]
    fn standard_mode_requires_password_and_stores_only_hash_material() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        let err = service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: None,
                two_factor_enabled: true,
                mnemonic_enabled: true,
            })
            .expect_err("missing password");
        assert_eq!(err.code(), ErrorCode::StorageEncryptionFailed);

        let status = service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("correct horse battery staple".to_string()),
                two_factor_enabled: true,
                mnemonic_enabled: true,
            })
            .expect("complete");
        assert!(status.password_required);
        assert!(!status.two_factor_enabled);
        assert!(status.two_factor_pending);
        let raw = fs::read_to_string(dir.path().join(CONFIG_FILE)).expect("config");
        assert!(!raw.contains("correct horse"));
        assert!(raw.contains("passwordHashHex"));
    }

    #[test]
    fn verifies_sensitive_actions_with_configured_password() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("correct horse battery staple".to_string()),
                two_factor_enabled: true,
                mnemonic_enabled: false,
            })
            .expect("complete");

        service
            .verify_sensitive_action(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: None,
            })
            .expect("verified");
        let err = service
            .verify_sensitive_action(SensitiveActionInput {
                password: Some("wrong password".to_string()),
                two_factor_code: None,
            })
            .expect_err("wrong password");
        assert_eq!(err.code(), ErrorCode::SecurityVerificationFailed);
    }

    #[test]
    fn enrolls_and_verifies_totp_for_sensitive_actions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("correct horse battery staple".to_string()),
                two_factor_enabled: true,
                mnemonic_enabled: false,
            })
            .expect("complete");

        let enrollment = service
            .begin_totp_enrollment(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: None,
            })
            .expect("begin");
        let secret = veil_secure_storage::decode_totp_secret(&enrollment.secret).unwrap();
        let code = veil_secure_storage::totp_code(&secret, current_unix_time()).unwrap();
        let status = service
            .confirm_totp_enrollment(
                &enrollment.secret,
                &code,
                SensitiveActionInput {
                    password: Some("correct horse battery staple".to_string()),
                    two_factor_code: None,
                },
            )
            .expect("confirm");
        assert!(status.two_factor_enabled);
        assert!(!status.two_factor_pending);

        service
            .verify_sensitive_action(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: Some(code.clone()),
            })
            .expect("verified");
        let err = service
            .verify_sensitive_action(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: None,
            })
            .expect_err("missing totp");
        assert_eq!(err.code(), ErrorCode::SecurityVerificationFailed);

        let status = service
            .disable_totp(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: Some(code),
            })
            .expect("disabled");
        assert!(!status.two_factor_enabled);
    }

    #[test]
    fn incomplete_onboarding_can_begin_totp_without_old_credentials() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());

        let enrollment = service
            .begin_totp_enrollment(SensitiveActionInput {
                password: Some("draft password".to_string()),
                two_factor_code: None,
            })
            .expect("begin");

        assert!(enrollment.otpauth_uri.starts_with("otpauth://totp/"));
        assert!(!enrollment.secret.is_empty());
    }

    #[test]
    fn incomplete_onboarding_can_confirm_totp_without_old_credentials() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        let enrollment = service
            .begin_totp_enrollment(SensitiveActionInput {
                password: Some("draft password".to_string()),
                two_factor_code: None,
            })
            .expect("begin");
        let secret = veil_secure_storage::decode_totp_secret(&enrollment.secret).unwrap();
        let code = veil_secure_storage::totp_code(&secret, current_unix_time()).unwrap();

        let status = service
            .confirm_totp_enrollment(
                &enrollment.secret,
                &code,
                SensitiveActionInput {
                    password: Some("draft password".to_string()),
                    two_factor_code: None,
                },
            )
            .expect("confirm");

        assert!(status.two_factor_enabled);
        assert!(!status.two_factor_pending);
    }

    #[test]
    fn preserves_verified_totp_secret_when_completion_repeats() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("correct horse battery staple".to_string()),
                two_factor_enabled: true,
                mnemonic_enabled: false,
            })
            .expect("complete");

        let enrollment = service
            .begin_totp_enrollment(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: None,
            })
            .expect("begin");
        let secret = veil_secure_storage::decode_totp_secret(&enrollment.secret).unwrap();
        let code = veil_secure_storage::totp_code(&secret, current_unix_time()).unwrap();
        service
            .confirm_totp_enrollment(
                &enrollment.secret,
                &code,
                SensitiveActionInput {
                    password: Some("correct horse battery staple".to_string()),
                    two_factor_code: None,
                },
            )
            .expect("confirm");

        let status = service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("correct horse battery staple".to_string()),
                two_factor_enabled: true,
                mnemonic_enabled: true,
            })
            .expect("complete again");
        assert!(status.two_factor_enabled);
        assert!(!status.two_factor_pending);
    }

    #[test]
    fn recovery_replaces_password_and_clears_existing_totp() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("correct horse battery staple".to_string()),
                two_factor_enabled: true,
                mnemonic_enabled: true,
            })
            .expect("complete");
        let enrollment = service
            .begin_totp_enrollment(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: None,
            })
            .expect("begin");
        let secret = veil_secure_storage::decode_totp_secret(&enrollment.secret).unwrap();
        let code = veil_secure_storage::totp_code(&secret, current_unix_time()).unwrap();
        service
            .confirm_totp_enrollment(
                &enrollment.secret,
                &code,
                SensitiveActionInput {
                    password: Some("correct horse battery staple".to_string()),
                    two_factor_code: None,
                },
            )
            .expect("confirm");

        let status = service
            .recover_security(RecoverSecurityInput {
                encryption_mode: EncryptionMode::Standard,
                password: Some("new correct horse".to_string()),
                two_factor_enabled: false,
            })
            .expect("recover");

        assert!(status.password_required);
        assert!(!status.two_factor_enabled);
        assert!(!status.two_factor_pending);
        service
            .verify_sensitive_action(SensitiveActionInput {
                password: Some("new correct horse".to_string()),
                two_factor_code: None,
            })
            .expect("new password works");
        let err = service
            .verify_sensitive_action(SensitiveActionInput {
                password: Some("correct horse battery staple".to_string()),
                two_factor_code: Some(code),
            })
            .expect_err("old credentials fail");
        assert_eq!(err.code(), ErrorCode::SecurityVerificationFailed);
    }

    #[test]
    fn recovery_security_validation_rejects_invalid_password() {
        let err = OnboardingService::validate_recover_security_input(&RecoverSecurityInput {
            encryption_mode: EncryptionMode::Standard,
            password: Some("short".to_string()),
            two_factor_enabled: false,
        })
        .expect_err("short password fails");

        assert_eq!(err.code(), ErrorCode::StorageEncryptionFailed);
    }

    #[test]
    fn reset_removes_onboarding_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        let service = OnboardingService::new(dir.path());
        service
            .complete(CompleteOnboardingInput {
                encryption_mode: EncryptionMode::None,
                password: None,
                two_factor_enabled: false,
                mnemonic_enabled: false,
            })
            .expect("complete");
        assert!(service.status().completed);

        let status = service.reset().expect("reset");
        assert!(!status.completed);
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }
}
