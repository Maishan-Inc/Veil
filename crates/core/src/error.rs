use std::fmt;

use serde::{Serialize, Serializer};

/// Stable, user-facing error codes. Each maps 1:1 to an i18n key (`errors.<code>`)
/// and to the TypeScript `AppErrorCode` union. Never surface raw error text to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // Runtime / engine
    RuntimeNetworkUnavailable,
    RuntimeGithubResponse,
    RuntimeAssetNotFound,
    RuntimeDownloadFailed,
    RuntimeDiskWriteFailed,
    RuntimeChecksumMismatch,
    RuntimeExtractFailed,
    RuntimeExecutableMissing,
    RuntimeRegistryWriteFailed,
    RuntimeInstallCancelled,
    RuntimeAlreadyInstalled,
    RuntimeCannotUninstallCurrent,
    // Launch
    LaunchRuntimeMissing,
    LaunchRuntimeExecutableMissing,
    LaunchProfileMissing,
    LaunchProfileInvalid,
    LaunchProcessSpawnFailed,
    /// The engine cannot start from where it is installed — distinct from a generic spawn failure
    /// because re-installing into the same location cannot fix it.
    LaunchEngineBlockedByLocation,
    LaunchUnsupportedArguments,
    // Profile
    ProfileNameRequired,
    ProfileNotFound,
    // Proxy
    ProxyNotFound,
    ProxyInvalid,
    ProxyTestFailed,
    // Extensions / plugins
    ExtensionNotFound,
    ExtensionInvalid,
    ExtensionDownloadFailed,
    // Fingerprint
    FingerprintInvalid,
    // AI / automation
    AiProviderMissing,
    AiAutomationUnavailable,
    AiTaskNotFound,
    AiSessionNotFound,
    AiApprovalRequired,
    // Local API
    LocalApiInvalidPort,
    LocalApiKeyNotFound,
    LocalApiBadRequest,
    LocalApiBindFailed,
    // Storage
    StorageDatabaseReadFailed,
    StorageDatabaseWriteFailed,
    StorageEncryptionFailed,
    StorageDecryptionFailed,
    StorageLocked,
    // Moving the engine root or the data directory
    MigrationTargetNotEmpty,
    MigrationNestedPaths,
    MigrationBusy,
    MigrationFailed,
    // Security / verification
    SecurityVerificationFailed,
    // Profile export / import
    ExportPassphraseRequired,
    ExportFormatInvalid,
    // Diagnostics
    DiagnosticsUnavailable,
    DiagnosticsExportFailed,
    // Fallback
    Unknown,
}

impl ErrorCode {
    /// The stable string form used by the UI / i18n layer.
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::RuntimeNetworkUnavailable => "runtime.networkUnavailable",
            ErrorCode::RuntimeGithubResponse => "runtime.githubResponse",
            ErrorCode::RuntimeAssetNotFound => "runtime.assetNotFound",
            ErrorCode::RuntimeDownloadFailed => "runtime.downloadFailed",
            ErrorCode::RuntimeDiskWriteFailed => "runtime.diskWriteFailed",
            ErrorCode::RuntimeChecksumMismatch => "runtime.checksumMismatch",
            ErrorCode::RuntimeExtractFailed => "runtime.extractFailed",
            ErrorCode::RuntimeExecutableMissing => "runtime.executableMissing",
            ErrorCode::RuntimeRegistryWriteFailed => "runtime.registryWriteFailed",
            ErrorCode::RuntimeInstallCancelled => "runtime.installCancelled",
            ErrorCode::RuntimeAlreadyInstalled => "runtime.alreadyInstalled",
            ErrorCode::RuntimeCannotUninstallCurrent => "runtime.cannotUninstallCurrent",
            ErrorCode::LaunchRuntimeMissing => "launch.runtimeMissing",
            ErrorCode::LaunchRuntimeExecutableMissing => "launch.runtimeExecutableMissing",
            ErrorCode::LaunchProfileMissing => "launch.profileMissing",
            ErrorCode::LaunchProfileInvalid => "launch.profileInvalid",
            ErrorCode::LaunchProcessSpawnFailed => "launch.processSpawnFailed",
            ErrorCode::LaunchEngineBlockedByLocation => "launch.engineBlockedByLocation",
            ErrorCode::LaunchUnsupportedArguments => "launch.unsupportedArguments",
            ErrorCode::ProfileNameRequired => "profile.nameRequired",
            ErrorCode::ProfileNotFound => "profile.notFound",
            ErrorCode::ProxyNotFound => "proxy.notFound",
            ErrorCode::ProxyInvalid => "proxy.invalid",
            ErrorCode::ProxyTestFailed => "proxy.testFailed",
            ErrorCode::ExtensionNotFound => "extension.notFound",
            ErrorCode::ExtensionInvalid => "extension.invalid",
            ErrorCode::ExtensionDownloadFailed => "extension.downloadFailed",
            ErrorCode::FingerprintInvalid => "fingerprint.invalid",
            ErrorCode::AiProviderMissing => "ai.providerMissing",
            ErrorCode::AiAutomationUnavailable => "ai.automationUnavailable",
            ErrorCode::AiTaskNotFound => "ai.taskNotFound",
            ErrorCode::AiSessionNotFound => "ai.sessionNotFound",
            ErrorCode::AiApprovalRequired => "ai.approvalRequired",
            ErrorCode::LocalApiInvalidPort => "localApi.invalidPort",
            ErrorCode::LocalApiKeyNotFound => "localApi.keyNotFound",
            ErrorCode::LocalApiBadRequest => "localApi.badRequest",
            ErrorCode::LocalApiBindFailed => "localApi.bindFailed",
            ErrorCode::StorageDatabaseReadFailed => "storage.databaseReadFailed",
            ErrorCode::StorageDatabaseWriteFailed => "storage.databaseWriteFailed",
            ErrorCode::StorageEncryptionFailed => "storage.encryptionFailed",
            ErrorCode::StorageDecryptionFailed => "storage.decryptionFailed",
            ErrorCode::StorageLocked => "storage.locked",
            ErrorCode::MigrationTargetNotEmpty => "migration.targetNotEmpty",
            ErrorCode::MigrationNestedPaths => "migration.nestedPaths",
            ErrorCode::MigrationBusy => "migration.busy",
            ErrorCode::MigrationFailed => "migration.failed",
            ErrorCode::SecurityVerificationFailed => "security.verificationFailed",
            ErrorCode::ExportPassphraseRequired => "export.passphraseRequired",
            ErrorCode::ExportFormatInvalid => "export.formatInvalid",
            ErrorCode::DiagnosticsUnavailable => "diagnostics.unavailable",
            ErrorCode::DiagnosticsExportFailed => "diagnostics.exportFailed",
            ErrorCode::Unknown => "app.unknown",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Application error carried across the service boundary. Serializes to `{ "code": "<dotted>" }`
/// so the frontend can localize it without ever seeing raw error text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    code: ErrorCode,
}

impl AppError {
    pub const fn new(code: ErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(&self) -> ErrorCode {
        self.code
    }
}

impl From<ErrorCode> for AppError {
    fn from(code: ErrorCode) -> Self {
        Self { code }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 1)?;
        state.serialize_field("code", self.code.as_str())?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_render_as_dotted_keys() {
        assert_eq!(
            ErrorCode::RuntimeChecksumMismatch.as_str(),
            "runtime.checksumMismatch"
        );
        assert_eq!(ErrorCode::StorageLocked.to_string(), "storage.locked");
    }

    #[test]
    fn app_error_serializes_with_code() {
        let json = serde_json::to_string(&AppError::new(ErrorCode::ProfileNotFound)).unwrap();
        assert_eq!(json, r#"{"code":"profile.notFound"}"#);
    }
}
