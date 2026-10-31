/**
 * Stable application error codes.
 *
 * Each value maps 1:1 to an i18n key (`errors.<code>`) and to a Rust
 * `veil_core::ErrorCode`. The backend returns the code string; the UI localizes it.
 * Never show raw exception text to users.
 */
export type AppErrorCode =
  // Runtime / engine
  | "runtime.networkUnavailable"
  | "runtime.githubResponse"
  | "runtime.assetNotFound"
  | "runtime.downloadFailed"
  | "runtime.diskWriteFailed"
  | "runtime.checksumMismatch"
  | "runtime.extractFailed"
  | "runtime.executableMissing"
  | "runtime.registryWriteFailed"
  | "runtime.installCancelled"
  | "runtime.alreadyInstalled"
  | "runtime.cannotUninstallCurrent"
  // Launch
  | "launch.runtimeMissing"
  | "launch.runtimeExecutableMissing"
  | "launch.profileMissing"
  | "launch.profileInvalid"
  | "launch.processSpawnFailed"
  | "launch.engineBlockedByLocation"
  | "launch.unsupportedArguments"
  // Profile
  | "profile.nameRequired"
  | "profile.notFound"
  // Proxy
  | "proxy.notFound"
  | "proxy.invalid"
  | "proxy.testFailed"
  // Extensions / plugins
  | "extension.notFound"
  | "extension.invalid"
  | "extension.downloadFailed"
  // Fingerprint
  | "fingerprint.invalid"
  // AI
  | "ai.providerMissing"
  | "ai.automationUnavailable"
  | "ai.taskNotFound"
  | "ai.sessionNotFound"
  | "ai.approvalRequired"
  // Local API
  | "localApi.invalidPort"
  | "localApi.keyNotFound"
  | "localApi.badRequest"
  | "localApi.bindFailed"
  // Storage
  | "storage.databaseReadFailed"
  | "storage.databaseWriteFailed"
  | "storage.encryptionFailed"
  | "storage.decryptionFailed"
  | "storage.locked"
  | "migration.targetNotEmpty"
  | "migration.nestedPaths"
  | "migration.busy"
  | "migration.failed"
  // Security
  | "security.verificationFailed"
  // Diagnostics
  | "diagnostics.unavailable"
  | "diagnostics.exportFailed"
  // Fallback
  | "app.unknown";
