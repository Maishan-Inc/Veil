import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AiAuditLog,
  AiAuditLogFilter,
  AiAutomationSession,
  AiModelConfig,
  AiProfileFact,
  AiSettings,
  AiTask,
  AiTaskEvent,
  AppErrorCode,
  AvailableRuntime,
  BrowserExtension,
  CreateProfileInput,
  CreateProxyInput,
  DataLocationInfo,
  EngineRootInfo,
  Fingerprint,
  Group,
  ImportExtensionFromLocalInput,
  ImportExtensionFromStoreInput,
  InstalledRuntimes,
  LaunchStatus,
  ProfileOS,
  Profile,
  Proxy,
  ProxyTestResult,
  RuntimeInstallState,
  RuntimeProgress,
  RuntimeVersion,
  RunAiTaskResult,
  StartAiTaskInput,
  StartAiTaskResult,
  UpdateAiSettingsInput,
  UpdateExtensionInput,
  UpsertAiModelConfigInput,
  UpsertAiProfileFactInput,
  ExportAiAuditLogsResult,
} from "@veil/shared";

export type EncryptionMode = "none" | "standard" | "advanced";

export interface OnboardingStatus {
  completed: boolean;
  agreementAcceptedAt: string | null;
  encryptionMode: EncryptionMode;
  passwordRequired: boolean;
  twoFactorEnabled: boolean;
  twoFactorPending: boolean;
  mnemonicEnabled: boolean;
  developer: string;
}

export interface CompleteOnboardingInput {
  encryptionMode: EncryptionMode;
  password: string | null;
  twoFactorEnabled: boolean;
  mnemonicEnabled: boolean;
}

export interface RecoverSecurityInput {
  encryptionMode: EncryptionMode;
  password: string | null;
  twoFactorEnabled: boolean;
}

export interface SensitiveActionInput {
  password: string | null;
  twoFactorCode: string | null;
}

export interface ExportProfilesResult {
  path: string;
  count: number;
}

export type ImportConflict = "skip" | "rename";

export interface ImportProfilesInput {
  path: string;
  /** Required for v2 (encrypted) exports; ignored for legacy v1 plaintext files. */
  passphrase: string | null;
  onConflict: ImportConflict;
}

export interface ImportProfilesResult {
  total: number;
  imported: number;
  renamed: number;
  skipped: number;
  failed: number;
}

export interface VaultStatus {
  locked: boolean;
  passwordRequired: boolean;
  wrapper: "dpapi" | "password";
}

export interface TotpEnrollment {
  secret: string;
  otpauthUri: string;
}

export interface RecoveryPhraseOutput {
  phrase: string[];
}

export interface ProxyCredentials {
  username: string | null;
  password: string | null;
}

export type LocalApiBindMode = "loopback" | "lan";

export interface LocalApiSettings {
  enabled: boolean;
  port: number;
  bindMode: LocalApiBindMode;
  updatedAt: string | null;
}

export interface UpdateLocalApiSettingsInput {
  enabled: boolean;
  port: number;
  bindMode: LocalApiBindMode;
}

export interface LocalApiKey {
  id: string;
  name: string;
  suffix: string;
  createdAt: string;
  updatedAt: string;
  lastUsedAt: string | null;
}

export interface LocalApiKeyIssue {
  key: LocalApiKey;
  fullKey: string;
}

export interface RunningProfile {
  profileId: string;
  processId: number;
}

export interface ProfileRunningState {
  profileId: string;
  running: boolean;
}

export interface DeleteProfileDataResult {
  profileId: string;
  deleted: boolean;
}

export interface ClearProfileCacheResult {
  profileId: string;
  clearedPaths: number;
}

export interface ProfileCookieJar {
  profileId: string;
  cookies: unknown;
  updatedAt: string;
}

export interface ProfileCredential {
  id: string;
  profileId: string;
  platform: string;
  username: string;
  password: string;
  notes: string;
  createdAt: string;
  updatedAt: string;
}

export interface OptimizeFingerprintForProxyInput {
  proxyId: string;
  os: CreateProfileInput["os"];
  seed: number;
}

export interface OptimizedFingerprintForProxyResult {
  fingerprint: Fingerprint;
  proxy: Proxy;
  test: ProxyTestResult;
}

export interface UpsertProfileCredentialInput {
  platform: string;
  username: string;
  password: string;
  notes: string;
}

export interface LaunchCustomTemporaryProfileInput {
  name: string;
  proxyId: string | null;
  os: CreateProfileInput["os"];
  platform: string;
  browserVersion: string;
  startupPage: CreateProfileInput["startupPage"];
  startupUrl: string;
  fingerprint: Fingerprint;
  extensionIds: string[];
}

export interface TemporaryProfileLaunchResult {
  sessionId: string;
  processId: number;
}

export interface CloseTemporaryProfileResult {
  sessionId: string;
  closed: boolean;
  dataDeleted: boolean;
}

/**
 * The only module that calls Tauri's `invoke`. Every other module talks to the backend
 * through these typed functions.
 *
 * Errors come back as `{ code: AppErrorCode }`. We unwrap to an `AppError` so callers can
 * `try/catch` and the UI can localize via the catalog. Raw exception text never reaches the UI.
 */
export class AppError extends Error {
  readonly code: AppErrorCode;
  constructor(code: AppErrorCode) {
    super(code);
    this.code = code;
    this.name = "AppError";
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toAppError(err);
  }
}

function toAppError(err: unknown): AppError {
  if (err && typeof err === "object" && "code" in err) {
    const code = (err as { code: unknown }).code;
    if (typeof code === "string") return new AppError(code as AppErrorCode);
  }
  if (typeof err === "string") return new AppError(err as AppErrorCode);
  return new AppError("app.unknown");
}

// ---- Runtime ----------------------------------------------------------------

export const getOnboardingStatus = (): Promise<OnboardingStatus> =>
  call<OnboardingStatus>("get_onboarding_status");

export const completeOnboarding = (input: CompleteOnboardingInput): Promise<OnboardingStatus> =>
  call<OnboardingStatus>("complete_onboarding", { input });

export const resetOnboarding = (): Promise<OnboardingStatus> =>
  call<OnboardingStatus>("reset_onboarding");

export const saveRecoveryPhraseTxt = (path: string, contents: string): Promise<void> =>
  call<void>("save_recovery_phrase_txt", { input: { path, contents } });

export const generateBackendRecoveryPhrase = (): Promise<RecoveryPhraseOutput> =>
  call<RecoveryPhraseOutput>("generate_recovery_phrase");

export const verifySensitiveAction = (input: SensitiveActionInput): Promise<void> =>
  call<void>("verify_sensitive_action", { input });

export const beginTotpEnrollment = (verification: SensitiveActionInput): Promise<TotpEnrollment> =>
  call<TotpEnrollment>("begin_totp_enrollment", { input: { verification } });

export const confirmTotpEnrollment = (
  secret: string,
  code: string,
  verification: SensitiveActionInput,
): Promise<OnboardingStatus> =>
  call<OnboardingStatus>("confirm_totp_enrollment", {
    input: { secret, code, verification },
  });

export const disableTotp = (input: SensitiveActionInput): Promise<OnboardingStatus> =>
  call<OnboardingStatus>("disable_totp", { input });

export const getVaultStatus = (): Promise<VaultStatus> => call<VaultStatus>("get_vault_status");

export const unlockVault = (password: string): Promise<VaultStatus> =>
  call<VaultStatus>("unlock_vault", { input: { password } });

export const recoverVaultWithRecoveryPhrase = (
  recoveryPhrase: string[],
  security: RecoverSecurityInput,
): Promise<VaultStatus> =>
  call<VaultStatus>("recover_vault_with_recovery_phrase", {
    input: { recoveryPhrase, security },
  });

export const getRuntimeState = (): Promise<RuntimeInstallState> =>
  call<RuntimeInstallState>("get_runtime_state");

export const getRuntimeInstallRoot = (): Promise<EngineRootInfo> =>
  call<EngineRootInfo>("get_runtime_install_root");

/**
 * Move the installed engines to `path` and remember it. Long-running: a cross-volume move copies
 * every installed version, so the caller must keep a busy affordance up until it resolves.
 */
export const setRuntimeInstallRoot = (path: string): Promise<EngineRootInfo> =>
  call<EngineRootInfo>("set_runtime_install_root", { path });

export const getDataLocation = (): Promise<DataLocationInfo> =>
  call<DataLocationInfo>("get_data_location");

/**
 * Record a data-directory move. It is applied at the next startup — the database is open while the
 * app runs — so a successful call means "queued", not "moved".
 */
export const setDataLocation = (path: string): Promise<DataLocationInfo> =>
  call<DataLocationInfo>("set_data_location", { path });

export const cancelDataLocationMove = (): Promise<DataLocationInfo> =>
  call<DataLocationInfo>("cancel_data_location_move");

export const restartApp = (): Promise<void> => call<void>("restart_app");

export const listInstalledRuntimes = (): Promise<InstalledRuntimes> =>
  call<InstalledRuntimes>("list_installed_runtimes");

export const listAvailableRuntimes = (): Promise<AvailableRuntime[]> =>
  call<AvailableRuntime[]>("list_available_runtimes");

/**
 * Install a runtime version. `onProgress` receives every backend event as it streams. The
 * returned promise resolves when the pipeline ends successfully, or rejects with an
 * `AppError` on failure / cancellation.
 */
export async function installRuntimeVersion(
  candidate: AvailableRuntime,
  onProgress: (progress: RuntimeProgress) => void,
): Promise<RuntimeVersion> {
  const channel = new Channel<RuntimeProgress>();
  channel.onmessage = onProgress;
  return call<RuntimeVersion>("install_runtime_version", { candidate, onProgress: channel });
}

export async function installRuntimeFromLocalZip(
  candidate: AvailableRuntime,
  localZipPath: string,
  onProgress: (progress: RuntimeProgress) => void,
): Promise<RuntimeVersion> {
  const channel = new Channel<RuntimeProgress>();
  channel.onmessage = onProgress;
  return call<RuntimeVersion>("install_runtime_from_local_zip", {
    candidate,
    localZipPath,
    onProgress: channel,
  });
}

export const cancelRuntimeInstall = (version: string): Promise<void> =>
  call<void>("cancel_runtime_install", { version });

export const setCurrentRuntime = (version: string): Promise<RuntimeInstallState> =>
  call<RuntimeInstallState>("set_current_runtime", { version });

export const uninstallRuntimeVersion = (version: string): Promise<void> =>
  call<void>("uninstall_runtime_version", { version });

// ---- Profiles ---------------------------------------------------------------

export const listProfiles = (): Promise<Profile[]> => call<Profile[]>("list_profiles");

export const getProfile = (profileId: string): Promise<Profile> =>
  call<Profile>("get_profile", { profileId });

export const createProfile = (input: CreateProfileInput): Promise<Profile> =>
  call<Profile>("create_profile", { input });

export const updateProfile = (profileId: string, input: CreateProfileInput): Promise<Profile> =>
  call<Profile>("update_profile", { profileId, input });

export const deleteProfile = (profileId: string): Promise<void> =>
  call<void>("delete_profile", { profileId });

export const listRunningProfiles = (): Promise<RunningProfile[]> =>
  call<RunningProfile[]>("list_running_profiles");

export const isProfileRunning = (profileId: string): Promise<ProfileRunningState> =>
  call<ProfileRunningState>("is_profile_running", { profileId });

export const exportProfiles = (
  profileIds: string[],
  passphrase: string,
  verification: SensitiveActionInput,
): Promise<ExportProfilesResult> =>
  call<ExportProfilesResult>("export_profiles", { profileIds, passphrase, verification });

/**
 * Import profiles from a Veil export file. IDs are never reused — every imported profile is
 * a new row; `onConflict` decides what happens to name collisions.
 */
export const importProfiles = (input: ImportProfilesInput): Promise<ImportProfilesResult> =>
  call<ImportProfilesResult>("import_profiles", { input });

/** Generate a coherent random fingerprint for `os`. `seed` makes it reproducible. */
export const randomizeFingerprint = (os: string, seed: number): Promise<Fingerprint> =>
  call<Fingerprint>("randomize_fingerprint", { os, seed });

/** Test the selected proxy and generate a fingerprint aligned to its detected region. */
export const optimizeFingerprintForProxy = (
  input: OptimizeFingerprintForProxyInput,
): Promise<OptimizedFingerprintForProxyResult> =>
  call<OptimizedFingerprintForProxyResult>("optimize_fingerprint_for_proxy", { input });

/**
 * Validate fingerprint consistency without saving; rejects with `fingerprint.invalid`.
 *
 * Takes the OS the dialog has selected because three of the backend's rules are about the machine the
 * profile claims — the display panel's Retina requirement, the work-area fit and the platform's shell
 * inset — and the claim lives on the profile's `os`, not in the fingerprint (5.5b).
 */
export const validateFingerprint = (fingerprint: Fingerprint, os: ProfileOS): Promise<void> =>
  call<void>("validate_fingerprint", { fingerprint, os });

export const updateProfileFingerprint = (
  profileId: string,
  fingerprint: Fingerprint,
): Promise<Profile> => call<Profile>("update_profile_fingerprint", { profileId, fingerprint });

export const deleteProfileData = (
  profileId: string,
  verification: SensitiveActionInput,
): Promise<DeleteProfileDataResult> =>
  call<DeleteProfileDataResult>("delete_profile_data", { profileId, verification });

export const clearProfileCache = (
  profileId: string,
  verification: SensitiveActionInput,
): Promise<ClearProfileCacheResult> =>
  call<ClearProfileCacheResult>("clear_profile_cache", { profileId, verification });

export const getProfileCookies = (
  profileId: string,
  verification: SensitiveActionInput,
): Promise<ProfileCookieJar> =>
  call<ProfileCookieJar>("get_profile_cookies", { profileId, verification });

export const updateProfileCookies = (
  profileId: string,
  cookies: unknown,
  verification: SensitiveActionInput,
): Promise<ProfileCookieJar> =>
  call<ProfileCookieJar>("update_profile_cookies", {
    profileId,
    input: { cookies },
    verification,
  });

export const listProfileCredentials = (
  profileId: string,
  verification: SensitiveActionInput,
): Promise<ProfileCredential[]> =>
  call<ProfileCredential[]>("list_profile_credentials", { profileId, verification });

export const createProfileCredential = (
  profileId: string,
  input: UpsertProfileCredentialInput,
  verification: SensitiveActionInput,
): Promise<ProfileCredential> =>
  call<ProfileCredential>("create_profile_credential", { profileId, input, verification });

export const updateProfileCredential = (
  credentialId: string,
  input: UpsertProfileCredentialInput,
  verification: SensitiveActionInput,
): Promise<ProfileCredential> =>
  call<ProfileCredential>("update_profile_credential", { credentialId, input, verification });

export const deleteProfileCredential = (
  credentialId: string,
  verification: SensitiveActionInput,
): Promise<void> => call<void>("delete_profile_credential", { credentialId, verification });

export const launchProfile = (profileId: string): Promise<LaunchStatus> =>
  call<LaunchStatus>("launch_profile", { profileId });

export const launchTemporaryProfile = (profileId: string): Promise<TemporaryProfileLaunchResult> =>
  call<TemporaryProfileLaunchResult>("launch_temporary_profile", { input: { profileId } });

export const launchCustomTemporaryProfile = (
  input: LaunchCustomTemporaryProfileInput,
): Promise<TemporaryProfileLaunchResult> =>
  call<TemporaryProfileLaunchResult>("launch_custom_temporary_profile", { input });

export const closeTemporaryProfile = (sessionId: string): Promise<CloseTemporaryProfileResult> =>
  call<CloseTemporaryProfileResult>("close_temporary_profile", { sessionId });

export const stopProfile = (profileId: string): Promise<Profile> =>
  call<Profile>("stop_profile", { profileId });

// ---- AI / automation --------------------------------------------------------

export const getAiSettings = (): Promise<AiSettings> => call<AiSettings>("get_ai_settings");

export const updateAiSettings = (input: UpdateAiSettingsInput): Promise<AiSettings> =>
  call<AiSettings>("update_ai_settings", { input });

export const hasAiProviderCredentials = (): Promise<boolean> =>
  call<boolean>("has_ai_provider_credentials");

export const listAiModelConfigs = (): Promise<AiModelConfig[]> =>
  call<AiModelConfig[]>("list_ai_model_configs");

export const upsertAiModelConfig = (input: UpsertAiModelConfigInput): Promise<AiModelConfig> =>
  call<AiModelConfig>("upsert_ai_model_config", { input });

export const listAiAuditLogs = (filter: AiAuditLogFilter): Promise<AiAuditLog[]> =>
  call<AiAuditLog[]>("list_ai_audit_logs", { filter });

export const exportAiAuditLogs = (
  filter: AiAuditLogFilter,
  verification: SensitiveActionInput,
): Promise<ExportAiAuditLogsResult> =>
  call<ExportAiAuditLogsResult>("export_ai_audit_logs", { filter, verification });

export const launchProfileForAi = (input: StartAiTaskInput): Promise<StartAiTaskResult> =>
  call<StartAiTaskResult>("launch_profile_for_ai", { input });

export const listAiTasks = (profileId: string): Promise<AiTask[]> =>
  call<AiTask[]>("list_ai_tasks", { profileId });

export const getAiTask = (taskId: string): Promise<AiTask> =>
  call<AiTask>("get_ai_task", { taskId });

export const listAiTaskEvents = (taskId: string): Promise<AiTaskEvent[]> =>
  call<AiTaskEvent[]>("list_ai_task_events", { taskId });

export const decideAiTaskApproval = (taskId: string, approved: boolean): Promise<AiTask> =>
  call<AiTask>("decide_ai_task_approval", { input: { taskId, approved } });

export const runAiTask = (taskId: string): Promise<RunAiTaskResult> =>
  call<RunAiTaskResult>("run_ai_task", { taskId });

export const pauseAiTaskForHumanVerification = (taskId: string): Promise<AiTask> =>
  call<AiTask>("pause_ai_task_for_human_verification", { taskId });

export const listAiProfileFacts = (profileId: string): Promise<AiProfileFact[]> =>
  call<AiProfileFact[]>("list_ai_profile_facts", { profileId });

export const upsertAiProfileFact = (input: UpsertAiProfileFactInput): Promise<AiProfileFact> =>
  call<AiProfileFact>("upsert_ai_profile_fact", { input });

export const getAiSession = (sessionId: string): Promise<AiAutomationSession> =>
  call<AiAutomationSession>("get_ai_session", { sessionId });

// ---- Local API --------------------------------------------------------------

export const getLocalApiSettings = (): Promise<LocalApiSettings> =>
  call<LocalApiSettings>("get_local_api_settings");

export const updateLocalApiSettings = (
  input: UpdateLocalApiSettingsInput,
  verification: SensitiveActionInput | null = null,
): Promise<LocalApiSettings> =>
  call<LocalApiSettings>("update_local_api_settings", { input, verification });

export const listLocalApiKeys = (): Promise<LocalApiKey[]> =>
  call<LocalApiKey[]>("list_local_api_keys");

export const generateLocalApiKey = (
  name: string,
  verification: SensitiveActionInput,
): Promise<LocalApiKeyIssue> =>
  call<LocalApiKeyIssue>("generate_local_api_key", { input: { name }, verification });

export const rotateLocalApiKey = (
  keyId: string,
  verification: SensitiveActionInput,
): Promise<LocalApiKeyIssue> =>
  call<LocalApiKeyIssue>("rotate_local_api_key", { input: { keyId }, verification });

// ---- Extensions / plugins --------------------------------------------------

export const listExtensions = (): Promise<BrowserExtension[]> =>
  call<BrowserExtension[]>("list_extensions");

export const getExtension = (extensionId: string): Promise<BrowserExtension> =>
  call<BrowserExtension>("get_extension", { extensionId });

export const importExtensionFromStore = (
  input: ImportExtensionFromStoreInput,
): Promise<BrowserExtension> => call<BrowserExtension>("import_extension_from_store", { input });

export const importExtensionFromLocal = (
  input: ImportExtensionFromLocalInput,
): Promise<BrowserExtension> => call<BrowserExtension>("import_extension_from_local", { input });

export const updateExtension = (
  extensionId: string,
  input: UpdateExtensionInput,
): Promise<BrowserExtension> => call<BrowserExtension>("update_extension", { extensionId, input });

export const deleteExtension = (extensionId: string): Promise<void> =>
  call<void>("delete_extension", { extensionId });

// ---- Groups -----------------------------------------------------------------

export const listGroups = (): Promise<Group[]> => call<Group[]>("list_groups");

export const createGroup = (input: { name: string; color: string | null }): Promise<Group> =>
  call<Group>("create_group", { input });

export const updateGroup = (
  groupId: string,
  input: { name: string; color: string | null },
): Promise<Group> => call<Group>("update_group", { groupId, input });

export const deleteGroup = (groupId: string, verification: SensitiveActionInput): Promise<void> =>
  call<void>("delete_group", { groupId, verification });

// ---- Proxies ----------------------------------------------------------------

/**
 * Update payload. `password`:
 * - `null` — leave the existing password untouched.
 * - non-empty string — replace.
 * - empty string — explicitly clear.
 */
export interface UpdateProxyInput extends Omit<CreateProxyInput, "password"> {
  password: string | null;
}

export const listProxies = (): Promise<Proxy[]> => call<Proxy[]>("list_proxies");

export const getProxy = (proxyId: string): Promise<Proxy> => call<Proxy>("get_proxy", { proxyId });

export const createProxy = (input: CreateProxyInput): Promise<Proxy> =>
  call<Proxy>("create_proxy", { input });

export const updateProxy = (proxyId: string, input: UpdateProxyInput): Promise<Proxy> =>
  call<Proxy>("update_proxy", { proxyId, input });

export const deleteProxy = (proxyId: string): Promise<void> =>
  call<void>("delete_proxy", { proxyId });

export const revealProxyCredentials = (
  proxyId: string,
  verification: SensitiveActionInput,
): Promise<ProxyCredentials> =>
  call<ProxyCredentials>("reveal_proxy_credentials", {
    input: { proxyId, verification },
  });

/** Returns the number of profiles whose assignment was successfully changed. */
export const batchSetProfileProxy = (
  profileIds: string[],
  proxyId: string | null,
): Promise<number> => call<number>("batch_set_profile_proxy", { profileIds, proxyId });

export const testProxy = (proxyId: string): Promise<ProxyTestResult> =>
  call<ProxyTestResult>("test_proxy", { proxyId });

// ---- Diagnostics ------------------------------------------------------------

export interface DiagnosticEvent {
  schemaVersion: number;
  seq: number;
  at: string;
  relMs: number;
  component: string;
  action: string;
  severity: string;
  outcome: string;
  code: string | null;
}

export interface DiagnosticsManifest {
  schemaVersion: number;
  appVersion: string;
  engineVersion: string | null;
  engineFlagDialect: string | null;
  windowsVersion: string;
  cpuModel: string | null;
  cpuCores: number | null;
  memoryBytes: number | null;
  gpus: string[];
  exportedAt: string;
  eventCount: number;
  includesGraphicsDetails: boolean;
}

export interface DiagnosticsBundle {
  manifest: DiagnosticsManifest;
  events: DiagnosticEvent[];
}

export interface ExportDiagnosticsResult {
  path: string;
  eventCount: number;
  bytes: number;
}

export interface ExportDiagnosticsInput {
  path: string;
  includeGraphicsDetails: boolean;
}

/** Build the diagnostic bundle the user previews before exporting. Local only. */
export const previewDiagnostics = (): Promise<DiagnosticsBundle> =>
  call<DiagnosticsBundle>("preview_diagnostics");

/** Write the diagnostic bundle to a user-chosen path. Local only — no upload. */
export const exportDiagnostics = (
  input: ExportDiagnosticsInput,
): Promise<ExportDiagnosticsResult> =>
  call<ExportDiagnosticsResult>("export_diagnostics", { input });

// ---- Environment ------------------------------------------------------------

/** True when we're inside a Tauri webview (vs. a plain browser preview). */
export const isTauriEnvironment = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
