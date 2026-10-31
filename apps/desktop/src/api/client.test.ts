import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => {
  class Channel<T> {
    onmessage: ((event: T) => void) | null = null;
  }
  return {
    Channel,
    invoke: vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === "get_runtime_state") return { kind: "missing" };
      if (command === "get_onboarding_status") {
        return {
          completed: false,
          agreementAcceptedAt: null,
          encryptionMode: "none",
          passwordRequired: false,
          twoFactorEnabled: false,
          twoFactorPending: false,
          mnemonicEnabled: false,
          developer: "Maishan Inc.",
        };
      }
      if (command === "complete_onboarding") {
        return {
          completed: true,
          agreementAcceptedAt: "2026-06-30T00:00:00Z",
          encryptionMode: (args?.input as { encryptionMode: string }).encryptionMode,
          passwordRequired: Boolean((args?.input as { password: string | null }).password),
          twoFactorEnabled: false,
          twoFactorPending: (args?.input as { twoFactorEnabled: boolean }).twoFactorEnabled,
          mnemonicEnabled: (args?.input as { mnemonicEnabled: boolean }).mnemonicEnabled,
          developer: "Maishan Inc.",
        };
      }
      if (command === "reset_onboarding") {
        return {
          completed: false,
          agreementAcceptedAt: null,
          encryptionMode: "none",
          passwordRequired: false,
          twoFactorEnabled: false,
          twoFactorPending: false,
          mnemonicEnabled: false,
          developer: "Maishan Inc.",
        };
      }
      if (command === "save_recovery_phrase_txt") return undefined;
      if (command === "generate_recovery_phrase") {
        return {
          phrase: [
            "AAAA",
            "BBBB",
            "CCCC",
            "DDDD",
            "EEEE",
            "FFFF",
            "GGGG",
            "HHHH",
            "IIII",
            "JJJJ",
            "KKKK",
            "LLLL",
          ],
        };
      }
      if (command === "verify_sensitive_action") {
        if ((args?.input as { password: string | null }).password === "wrong") {
          throw { code: "security.verificationFailed" };
        }
        return undefined;
      }
      if (command === "begin_totp_enrollment") {
        return {
          secret: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
          otpauthUri: "otpauth://totp/Veil:Veil?secret=GEZD",
        };
      }
      if (command === "confirm_totp_enrollment") {
        if ((args?.input as { code: string }).code === "000000") {
          throw { code: "security.verificationFailed" };
        }
        return {
          completed: true,
          agreementAcceptedAt: "2026-06-30T00:00:00Z",
          encryptionMode: "standard",
          passwordRequired: true,
          twoFactorEnabled: true,
          twoFactorPending: false,
          mnemonicEnabled: false,
          developer: "Maishan Inc.",
        };
      }
      if (command === "disable_totp") {
        return {
          completed: true,
          agreementAcceptedAt: "2026-06-30T00:00:00Z",
          encryptionMode: "standard",
          passwordRequired: true,
          twoFactorEnabled: false,
          twoFactorPending: false,
          mnemonicEnabled: false,
          developer: "Maishan Inc.",
        };
      }
      if (command === "get_vault_status") {
        return { locked: true, passwordRequired: true, wrapper: "password" };
      }
      if (command === "unlock_vault") {
        if ((args?.input as { password: string }).password === "wrong") {
          throw { code: "security.verificationFailed" };
        }
        return { locked: false, passwordRequired: true, wrapper: "password" };
      }
      if (command === "recover_vault_with_recovery_phrase") {
        if ((args?.input as { recoveryPhrase: string[] }).recoveryPhrase[0] === "WRONG") {
          throw { code: "security.verificationFailed" };
        }
        return {
          locked: false,
          passwordRequired: Boolean(
            (args?.input as { security: { password: string | null } }).security.password,
          ),
          wrapper: (args?.input as { security: { password: string | null } }).security.password
            ? "password"
            : "dpapi",
        };
      }
      if (command === "export_profiles") {
        return {
          path: "C:\\Users\\user\\AppData\\Local\\Veil\\exports\\profiles.json",
          count: (args?.profileIds as string[]).length,
        };
      }
      if (command === "import_profiles") {
        const input = args?.input as {
          path: string;
          passphrase: string | null;
          onConflict: string;
        };
        if (!input.path) throw { code: "export.formatInvalid" };
        return { total: 2, imported: 1, renamed: 1, skipped: 0, failed: 0 };
      }
      if (command === "list_profiles") return [];
      if (command === "list_running_profiles") {
        return [{ profileId: "profile-1", processId: 42 }];
      }
      if (command === "is_profile_running") {
        return { profileId: args?.profileId, running: args?.profileId === "profile-1" };
      }
      if (command === "update_profile_fingerprint") {
        return { id: args?.profileId, fingerprint: args?.fingerprint };
      }
      if (command === "optimize_fingerprint_for_proxy") {
        const fingerprint = { ...emptyFingerprint(), locale: "ja-JP", timezone: "Asia/Tokyo" };
        return {
          fingerprint,
          proxy: {
            id: (args?.input as { proxyId: string }).proxyId,
            name: "Tokyo",
            type: "http",
            host: "127.0.0.1",
            port: 8080,
            username: null,
            hasPassword: false,
            country: "Japan",
            detectedIp: "203.0.113.10",
            countryCode: "JP",
            region: "Tokyo",
            city: "Tokyo",
            timezone: "Asia/Tokyo",
            createdAt: "2026-07-03T00:00:00Z",
            updatedAt: "2026-07-03T00:00:00Z",
          },
          test: {
            ok: true,
            ip: "203.0.113.10",
            country: "Japan",
            countryCode: "JP",
            region: "Tokyo",
            city: "Tokyo",
            timezone: "Asia/Tokyo",
            latencyMs: 50,
            code: null,
          },
        };
      }
      if (command === "delete_profile_data") {
        if ((args?.verification as { password: string | null }).password === "wrong") {
          throw { code: "security.verificationFailed" };
        }
        return { profileId: args?.profileId, deleted: true };
      }
      if (command === "clear_profile_cache") {
        return { profileId: args?.profileId, clearedPaths: 3 };
      }
      if (command === "get_profile_cookies") {
        return {
          profileId: args?.profileId,
          cookies: [{ name: "sid", value: "secret" }],
          updatedAt: "2026-07-03T00:00:00Z",
        };
      }
      if (command === "update_profile_cookies") {
        return {
          profileId: args?.profileId,
          cookies: (args?.input as { cookies: unknown }).cookies,
          updatedAt: "2026-07-03T00:00:00Z",
        };
      }
      if (command === "list_profile_credentials") {
        return [
          {
            id: "credential-1",
            profileId: args?.profileId,
            platform: "example.com",
            username: "user",
            password: "secret",
            notes: "",
            createdAt: "2026-07-03T00:00:00Z",
            updatedAt: "2026-07-03T00:00:00Z",
          },
        ];
      }
      if (command === "create_profile_credential") {
        return {
          id: "credential-2",
          profileId: args?.profileId,
          ...(args?.input as Record<string, unknown>),
          createdAt: "2026-07-03T00:00:00Z",
          updatedAt: "2026-07-03T00:00:00Z",
        };
      }
      if (command === "update_profile_credential") {
        return {
          id: args?.credentialId,
          profileId: "profile-1",
          ...(args?.input as Record<string, unknown>),
          createdAt: "2026-07-03T00:00:00Z",
          updatedAt: "2026-07-03T00:00:00Z",
        };
      }
      if (command === "delete_profile_credential") return undefined;
      if (command === "launch_temporary_profile") {
        return { sessionId: "temp-1", processId: 43 };
      }
      if (command === "launch_custom_temporary_profile") {
        return { sessionId: "temp-2", processId: 44 };
      }
      if (command === "close_temporary_profile") {
        return { sessionId: args?.sessionId, closed: true, dataDeleted: true };
      }
      if (command === "reveal_proxy_credentials") {
        if (
          (args?.input as { verification: { password: string | null } }).verification.password ===
          "wrong"
        ) {
          throw { code: "security.verificationFailed" };
        }
        return { username: "user", password: "secret" };
      }
      if (command === "list_installed_runtimes") {
        return {
          currentVersion: null,
          installed: [],
          engineName: "ungoogled-chromium",
          flagDialect: "stock-chromium",
          dialectRecorded: false,
        };
      }
      if (command === "list_available_runtimes") {
        throw { code: "runtime.assetNotFound" };
      }
      if (command === "install_runtime_from_local_zip") {
        return {
          version: (args?.candidate as { version: string }).version,
          channel: "free",
          executablePath: "C:/Veil/runtime/120/chrome.exe",
          installedAt: "2026-06-30T00:00:00.000Z",
          sha256: "a".repeat(64),
          sizeBytes: 42,
        };
      }
      if (command === "get_profile") {
        if (args?.profileId === "missing") {
          throw { code: "profile.notFound" };
        }
        return { id: args?.profileId };
      }
      if (command === "get_ai_settings") {
        return {
          providerKind: "openai",
          baseUrl: "https://api.openai.com/v1",
          model: "gpt-5.5",
          reasoningEffort: "medium",
          reviewMode: "reviewed",
          networkEnabled: false,
          hasApiKey: false,
          updatedAt: null,
        };
      }
      if (command === "update_ai_settings") {
        const input = args?.input as {
          providerKind: string;
          baseUrl: string;
          model: string;
          reasoningEffort: string;
          reviewMode: string;
          networkEnabled: boolean;
          apiKey: string | null;
        };
        return {
          providerKind: input.providerKind,
          baseUrl: input.baseUrl,
          model: input.model,
          reasoningEffort: input.reasoningEffort,
          reviewMode: input.reviewMode,
          networkEnabled: input.networkEnabled,
          hasApiKey: Boolean(input.apiKey),
          updatedAt: "2026-07-01T00:00:00Z",
        };
      }
      if (command === "has_ai_provider_credentials") return true;
      if (command === "list_ai_model_configs") {
        return [
          {
            id: "model-1",
            name: "Default",
            providerKind: "openai",
            icon: "openai",
            baseUrl: "https://api.openai.com/v1",
            model: "gpt-5.5",
            reasoningEffort: "medium",
            reviewMode: "reviewed",
            networkEnabled: true,
            hasApiKey: true,
            isDefault: true,
            createdAt: "2026-07-01T00:00:00Z",
            updatedAt: "2026-07-01T00:00:00Z",
          },
        ];
      }
      if (command === "upsert_ai_model_config") {
        const input = args?.input as {
          id: string | null;
          name: string;
          providerKind: string;
          icon: string;
          baseUrl: string;
          model: string;
          reasoningEffort: string;
          reviewMode: string;
          networkEnabled: boolean;
          apiKey: string | null;
          isDefault: boolean;
        };
        return {
          id: input.id ?? "model-2",
          name: input.name,
          providerKind: input.providerKind,
          icon: input.icon,
          baseUrl: input.baseUrl,
          model: input.model,
          reasoningEffort: input.reasoningEffort,
          reviewMode: input.reviewMode,
          networkEnabled: input.networkEnabled,
          hasApiKey: Boolean(input.apiKey),
          isDefault: input.isDefault,
          createdAt: "2026-07-01T00:00:00Z",
          updatedAt: "2026-07-01T00:00:00Z",
        };
      }
      if (command === "list_ai_audit_logs") return [];
      if (command === "export_ai_audit_logs") {
        if ((args?.verification as { password: string | null }).password === "wrong") {
          throw { code: "security.verificationFailed" };
        }
        return {
          path: "C:\\Users\\user\\AppData\\Local\\Veil\\exports\\ai-audit.csv",
          count: 0,
        };
      }
      if (command === "launch_profile_for_ai") {
        const input = args?.input as { profileId: string; prompt: string; executionMode: string };
        return {
          task: {
            id: "task-1",
            profileId: input.profileId,
            prompt: input.prompt,
            executionMode: input.executionMode,
            status: "awaitingApproval",
            sessionId: "session-1",
            cdpUrl: null,
            requiresUserAction: true,
            pendingApprovalCode: "task.start",
            pendingApprovalMessage: "Approve task",
            humanVerificationPolicy: "retryOnceThenPause",
            paymentPolicy: "userRequired",
            createdAt: "2026-07-01T00:00:00Z",
            updatedAt: "2026-07-01T00:00:00Z",
          },
          session: {
            sessionId: "session-1",
            profileId: input.profileId,
            processId: 42,
            createdAt: "2026-07-01T00:00:00Z",
          },
        };
      }
      if (command === "list_ai_tasks") return [];
      if (command === "list_ai_task_events") return [];
      if (command === "decide_ai_task_approval") {
        return { id: (args?.input as { taskId: string }).taskId, status: "ready" };
      }
      if (command === "run_ai_task") {
        return { task: { id: args?.taskId, status: "running" }, started: true };
      }
      if (command === "pause_ai_task_for_human_verification") {
        return { id: args?.taskId, status: "pausedForUser" };
      }
      if (command === "list_ai_profile_facts") return [];
      if (command === "upsert_ai_profile_fact") {
        const input = args?.input as { profileId: string; key: string; value: string };
        return { id: "fact-1", profileId: input.profileId, key: input.key, value: input.value };
      }
      if (command === "get_ai_session") {
        return { sessionId: args?.sessionId, profileId: "profile-1" };
      }
      if (command === "get_local_api_settings") {
        return { enabled: false, port: 17688, bindMode: "loopback", updatedAt: null };
      }
      if (command === "update_local_api_settings") {
        const input = args?.input as { enabled: boolean; port: number; bindMode: string };
        if (input.port < 1024) throw { code: "localApi.invalidPort" };
        return { ...input, updatedAt: "2026-07-03T00:00:00Z" };
      }
      if (command === "list_local_api_keys") {
        return [
          {
            id: "key-1",
            name: "Default",
            suffix: "ABC123",
            createdAt: "2026-07-03T00:00:00Z",
            updatedAt: "2026-07-03T00:00:00Z",
            lastUsedAt: null,
          },
        ];
      }
      if (command === "generate_local_api_key") {
        if ((args?.verification as { password: string | null }).password === "wrong") {
          throw { code: "security.verificationFailed" };
        }
        return {
          key: {
            id: "key-2",
            name: (args?.input as { name: string }).name,
            suffix: "XYZ789",
            createdAt: "2026-07-03T00:00:00Z",
            updatedAt: "2026-07-03T00:00:00Z",
            lastUsedAt: null,
          },
          fullKey: "veil_generated_XYZ789",
        };
      }
      if (command === "rotate_local_api_key") {
        if ((args?.verification as { password: string | null }).password === "wrong") {
          throw { code: "security.verificationFailed" };
        }
        return {
          key: {
            id: (args?.input as { keyId: string }).keyId,
            name: "Default",
            suffix: "ROTATE",
            createdAt: "2026-07-03T00:00:00Z",
            updatedAt: "2026-07-03T00:00:00Z",
            lastUsedAt: null,
          },
          fullKey: "veil_rotated_ROTATE",
        };
      }
      if (command === "list_extensions") return [];
      if (command === "import_extension_from_store") {
        const input = args?.input as {
          storeUrl: string;
          defaultEnabled: boolean;
          profileIds: string[];
        };
        return {
          id: "extension-1",
          name: "Imported",
          source: "chromeStore",
          storeUrl: input.storeUrl,
          storeId: "cjpalhdlnbpafiamejdnhcphjbkeiagm",
          version: "1.0.0",
          installPath: "C:/Veil/extensions/extension-1",
          defaultEnabled: input.defaultEnabled,
          profileIds: input.profileIds,
          createdAt: "2026-07-01T00:00:00Z",
          updatedAt: "2026-07-01T00:00:00Z",
        };
      }
      if (command === "import_extension_from_local") {
        const input = args?.input as {
          localPath: string;
          defaultEnabled: boolean;
          profileIds: string[];
        };
        return {
          id: "extension-2",
          name: "Local",
          source: "local",
          storeUrl: null,
          storeId: null,
          version: "1.0.0",
          installPath: input.localPath,
          defaultEnabled: input.defaultEnabled,
          profileIds: input.profileIds,
          createdAt: "2026-07-01T00:00:00Z",
          updatedAt: "2026-07-01T00:00:00Z",
        };
      }
      if (command === "update_extension") {
        const input = args?.input as {
          name: string;
          defaultEnabled: boolean;
          profileIds: string[];
        };
        return {
          id: args?.extensionId,
          name: input.name,
          source: "local",
          storeUrl: null,
          storeId: null,
          version: "1.0.0",
          installPath: "C:/Veil/extensions/extension-2",
          defaultEnabled: input.defaultEnabled,
          profileIds: input.profileIds,
          createdAt: "2026-07-01T00:00:00Z",
          updatedAt: "2026-07-01T00:00:00Z",
        };
      }
      if (command === "delete_extension") return undefined;
      if (command === "list_groups") return [];
      if (command === "create_group") {
        return { id: "group-1", ...(args?.input as Record<string, unknown>) };
      }
      if (command === "update_group") {
        return { id: args?.groupId, ...(args?.input as Record<string, unknown>) };
      }
      if (command === "delete_group") return undefined;
      throw new Error(command);
    }),
  };
});

import { emptyFingerprint } from "@veil/shared";
import {
  AppError,
  beginTotpEnrollment,
  clearProfileCache,
  closeTemporaryProfile,
  completeOnboarding,
  confirmTotpEnrollment,
  createGroup,
  createProfileCredential,
  deleteGroup,
  deleteProfileCredential,
  deleteProfileData,
  disableTotp,
  exportProfiles,
  generateBackendRecoveryPhrase,
  getProfile,
  getProfileCookies,
  getOnboardingStatus,
  getRuntimeState,
  getVaultStatus,
  importProfiles,
  isProfileRunning,
  installRuntimeFromLocalZip,
  getAiSettings,
  updateAiSettings,
  hasAiProviderCredentials,
  listAiModelConfigs,
  upsertAiModelConfig,
  listAiAuditLogs,
  exportAiAuditLogs,
  launchProfileForAi,
  listAiTasks,
  listAiTaskEvents,
  decideAiTaskApproval,
  runAiTask,
  pauseAiTaskForHumanVerification,
  listAiProfileFacts,
  upsertAiProfileFact,
  getAiSession,
  getLocalApiSettings,
  importExtensionFromLocal,
  importExtensionFromStore,
  generateLocalApiKey,
  launchCustomTemporaryProfile,
  listExtensions,
  listLocalApiKeys,
  listAvailableRuntimes,
  deleteExtension,
  listInstalledRuntimes,
  listProfileCredentials,
  listProfiles,
  listRunningProfiles,
  launchTemporaryProfile,
  optimizeFingerprintForProxy,
  revealProxyCredentials,
  resetOnboarding,
  recoverVaultWithRecoveryPhrase,
  rotateLocalApiKey,
  saveRecoveryPhraseTxt,
  unlockVault,
  updateExtension,
  updateGroup,
  updateLocalApiSettings,
  updateProfileCredential,
  updateProfileCookies,
  updateProfileFingerprint,
  verifySensitiveAction,
} from "./client";

describe("api client", () => {
  it("returns typed runtime state", async () => {
    await expect(getRuntimeState()).resolves.toEqual({ kind: "missing" });
  });

  it("returns onboarding status", async () => {
    await expect(getOnboardingStatus()).resolves.toMatchObject({
      completed: false,
      developer: "Maishan Inc.",
    });
  });

  it("completes onboarding with local security choices", async () => {
    await expect(
      completeOnboarding({
        encryptionMode: "standard",
        password: "correct horse battery staple",
        twoFactorEnabled: true,
        mnemonicEnabled: true,
      }),
    ).resolves.toMatchObject({
      completed: true,
      encryptionMode: "standard",
      passwordRequired: true,
    });
  });

  it("resets onboarding and verifies sensitive actions", async () => {
    await expect(resetOnboarding()).resolves.toMatchObject({ completed: false });
    await expect(
      verifySensitiveAction({ password: "correct horse battery staple", twoFactorCode: null }),
    ).resolves.toBeUndefined();
    await expect(
      verifySensitiveAction({ password: "wrong", twoFactorCode: null }),
    ).rejects.toMatchObject({
      code: "security.verificationFailed",
    });
  });

  it("reads and unlocks vault status", async () => {
    await expect(getVaultStatus()).resolves.toEqual({
      locked: true,
      passwordRequired: true,
      wrapper: "password",
    });
    await expect(unlockVault("correct horse battery staple")).resolves.toEqual({
      locked: false,
      passwordRequired: true,
      wrapper: "password",
    });
    await expect(unlockVault("wrong")).rejects.toMatchObject({
      code: "security.verificationFailed",
    });
  });

  it("saves recovery phrase txt through the backend", async () => {
    await expect(
      saveRecoveryPhraseTxt("C:/Users/Test/Desktop/veil-recovery.txt", "AAAA BBBB"),
    ).resolves.toBeUndefined();
  });

  it("generates a recovery phrase and recovers the vault with new security settings", async () => {
    await expect(generateBackendRecoveryPhrase()).resolves.toMatchObject({
      phrase: expect.arrayContaining(["AAAA", "LLLL"]),
    });
    await expect(
      recoverVaultWithRecoveryPhrase(["AAAA", "BBBB"], {
        encryptionMode: "standard",
        password: "new correct horse",
        twoFactorEnabled: true,
      }),
    ).resolves.toEqual({
      locked: false,
      passwordRequired: true,
      wrapper: "password",
    });
    await expect(
      recoverVaultWithRecoveryPhrase(["WRONG"], {
        encryptionMode: "none",
        password: null,
        twoFactorEnabled: false,
      }),
    ).rejects.toMatchObject({ code: "security.verificationFailed" });
  });

  it("enrolls and disables TOTP", async () => {
    await expect(
      beginTotpEnrollment({ password: "correct horse battery staple", twoFactorCode: null }),
    ).resolves.toMatchObject({
      secret: expect.stringContaining("GEZD"),
      otpauthUri: expect.stringContaining("otpauth://"),
    });
    await expect(
      confirmTotpEnrollment("GEZD", "123456", {
        password: "correct horse battery staple",
        twoFactorCode: null,
      }),
    ).resolves.toMatchObject({ twoFactorEnabled: true });
    await expect(
      confirmTotpEnrollment("GEZD", "000000", {
        password: "correct horse battery staple",
        twoFactorCode: null,
      }),
    ).rejects.toMatchObject({ code: "security.verificationFailed" });
    await expect(
      disableTotp({ password: "correct horse battery staple", twoFactorCode: "123456" }),
    ).resolves.toMatchObject({ twoFactorEnabled: false });
  });

  it("exports selected profiles with verification material", async () => {
    await expect(
      exportProfiles(["a", "b"], "correct horse battery staple", {
        password: "correct horse battery staple",
        twoFactorCode: null,
      }),
    ).resolves.toMatchObject({
      count: 2,
      path: expect.stringContaining("profiles.json"),
    });
  });

  it("imports profiles from an export file", async () => {
    await expect(
      importProfiles({
        path: "C:\\Downloads\\profiles-2026-09-06.json",
        passphrase: "correct horse battery staple",
        onConflict: "rename",
      }),
    ).resolves.toMatchObject({ total: 2, imported: 1, renamed: 1, failed: 0 });
  });

  it("returns an empty profile list", async () => {
    await expect(listProfiles()).resolves.toEqual([]);
  });

  it("wraps expanded profile API commands", async () => {
    const verification = { password: "correct horse battery staple", twoFactorCode: null };
    const fingerprint = emptyFingerprint();
    await expect(listRunningProfiles()).resolves.toEqual([
      { profileId: "profile-1", processId: 42 },
    ]);
    await expect(isProfileRunning("profile-1")).resolves.toEqual({
      profileId: "profile-1",
      running: true,
    });
    await expect(updateProfileFingerprint("profile-1", fingerprint)).resolves.toMatchObject({
      id: "profile-1",
      fingerprint,
    });
    await expect(
      optimizeFingerprintForProxy({ proxyId: "proxy-1", os: "windows", seed: 42 }),
    ).resolves.toMatchObject({
      fingerprint: { locale: "ja-JP", timezone: "Asia/Tokyo" },
      proxy: { countryCode: "JP", city: "Tokyo" },
      test: { ok: true, countryCode: "JP" },
    });
    await expect(deleteProfileData("profile-1", verification)).resolves.toEqual({
      profileId: "profile-1",
      deleted: true,
    });
    await expect(
      deleteProfileData("profile-1", { ...verification, password: "wrong" }),
    ).rejects.toMatchObject({
      code: "security.verificationFailed",
    });
    await expect(clearProfileCache("profile-1", verification)).resolves.toEqual({
      profileId: "profile-1",
      clearedPaths: 3,
    });
    await expect(getProfileCookies("profile-1", verification)).resolves.toMatchObject({
      profileId: "profile-1",
      cookies: [{ name: "sid", value: "secret" }],
    });
    await expect(
      updateProfileCookies("profile-1", [{ name: "sid", value: "new" }], verification),
    ).resolves.toMatchObject({
      profileId: "profile-1",
      cookies: [{ name: "sid", value: "new" }],
    });
    await expect(listProfileCredentials("profile-1", verification)).resolves.toMatchObject([
      { id: "credential-1", platform: "example.com", username: "user" },
    ]);
    await expect(
      createProfileCredential(
        "profile-1",
        { platform: "example.com", username: "user", password: "secret", notes: "" },
        verification,
      ),
    ).resolves.toMatchObject({ id: "credential-2", profileId: "profile-1" });
    await expect(
      updateProfileCredential(
        "credential-1",
        { platform: "example.org", username: "new", password: "secret2", notes: "updated" },
        verification,
      ),
    ).resolves.toMatchObject({ id: "credential-1", platform: "example.org" });
    await expect(deleteProfileCredential("credential-1", verification)).resolves.toBeUndefined();
  });

  it("wraps temporary profile and group commands", async () => {
    const verification = { password: "correct horse battery staple", twoFactorCode: null };
    await expect(launchTemporaryProfile("profile-1")).resolves.toEqual({
      sessionId: "temp-1",
      processId: 43,
    });
    await expect(
      launchCustomTemporaryProfile({
        name: "One-time",
        proxyId: null,
        os: "windows",
        platform: "Storefront",
        browserVersion: "",
        startupPage: "veil",
        startupUrl: "",
        fingerprint: emptyFingerprint(),
        extensionIds: [],
      }),
    ).resolves.toEqual({ sessionId: "temp-2", processId: 44 });
    await expect(closeTemporaryProfile("temp-1")).resolves.toEqual({
      sessionId: "temp-1",
      closed: true,
      dataDeleted: true,
    });
    await expect(createGroup({ name: "Team", color: "#5640c9" })).resolves.toMatchObject({
      id: "group-1",
      name: "Team",
    });
    await expect(updateGroup("group-1", { name: "Team 2", color: null })).resolves.toMatchObject({
      id: "group-1",
      name: "Team 2",
    });
    await expect(deleteGroup("group-1", verification)).resolves.toBeUndefined();
  });

  it("reveals proxy credentials after verification", async () => {
    await expect(
      revealProxyCredentials("proxy-1", {
        password: "correct horse battery staple",
        twoFactorCode: "123456",
      }),
    ).resolves.toEqual({ username: "user", password: "secret" });
    await expect(
      revealProxyCredentials("proxy-1", { password: "wrong", twoFactorCode: "123456" }),
    ).rejects.toMatchObject({ code: "security.verificationFailed" });
  });

  it("returns an empty installed list alongside the engine's flag dialect", async () => {
    await expect(listInstalledRuntimes()).resolves.toEqual({
      currentVersion: null,
      installed: [],
      engineName: "ungoogled-chromium",
      flagDialect: "stock-chromium",
      dialectRecorded: false,
    });
  });

  it("surfaces runtime.assetNotFound when the manifest is a placeholder", async () => {
    await expect(listAvailableRuntimes()).rejects.toMatchObject({
      name: "AppError",
      code: "runtime.assetNotFound",
    });
  });

  it("installs a runtime from a local zip path", async () => {
    await expect(
      installRuntimeFromLocalZip(
        {
          version: "120",
          assetName: "engine.zip",
          downloadUrl: "https://example.invalid/engine.zip",
          sha256: "a".repeat(64),
          sizeBytes: 42,
          publishedAt: null,
        },
        "D:\\Downloads\\engine.zip",
        () => undefined,
      ),
    ).resolves.toMatchObject({
      version: "120",
      executablePath: "C:/Veil/runtime/120/chrome.exe",
    });
  });

  it("rethrows backend errors as AppError with their code", async () => {
    await expect(getProfile("missing")).rejects.toMatchObject({
      name: "AppError",
      code: "profile.notFound",
    });
    await expect(getProfile("missing")).rejects.toBeInstanceOf(AppError);
  });

  it("wraps AI automation commands", async () => {
    await expect(getAiSettings()).resolves.toMatchObject({ model: "gpt-5.5" });
    await expect(
      updateAiSettings({
        providerKind: "openai",
        baseUrl: "https://api.openai.com/v1",
        model: "gpt-5.5",
        reasoningEffort: "medium",
        reviewMode: "reviewed",
        networkEnabled: true,
        apiKey: "secret",
      }),
    ).resolves.toMatchObject({ hasApiKey: true });
    await expect(hasAiProviderCredentials()).resolves.toBe(true);
    await expect(listAiModelConfigs()).resolves.toMatchObject([{ id: "model-1" }]);
    await expect(
      upsertAiModelConfig({
        id: null,
        name: "Local model",
        providerKind: "openaiCompatible",
        icon: "custom",
        baseUrl: "http://localhost:11434/v1",
        model: "llama3",
        reasoningEffort: "medium",
        reviewMode: "reviewed",
        networkEnabled: true,
        apiKey: "local",
        isDefault: false,
      }),
    ).resolves.toMatchObject({ id: "model-2", providerKind: "openaiCompatible" });
    await expect(listAiAuditLogs({ startAt: null, endAt: null, actor: "all" })).resolves.toEqual(
      [],
    );
    await expect(
      exportAiAuditLogs(
        { startAt: null, endAt: null, actor: "all" },
        { password: "correct horse battery staple", twoFactorCode: null },
      ),
    ).resolves.toMatchObject({ count: 0, path: expect.stringContaining("ai-audit.csv") });
    await expect(
      launchProfileForAi({
        profileId: "profile-1",
        modelConfigId: null,
        prompt: "register",
        executionMode: "reviewed",
      }),
    ).resolves.toMatchObject({ task: { id: "task-1" }, session: { sessionId: "session-1" } });
    await expect(listAiTasks("profile-1")).resolves.toEqual([]);
    await expect(listAiTaskEvents("task-1")).resolves.toEqual([]);
    await expect(decideAiTaskApproval("task-1", true)).resolves.toMatchObject({ status: "ready" });
    await expect(runAiTask("task-1")).resolves.toMatchObject({ started: true });
    await expect(pauseAiTaskForHumanVerification("task-1")).resolves.toMatchObject({
      status: "pausedForUser",
    });
    await expect(listAiProfileFacts("profile-1")).resolves.toEqual([]);
    await expect(
      upsertAiProfileFact({
        profileId: "profile-1",
        key: "gmail.email",
        value: "a@gmail.com",
        source: "manual",
        sensitivity: "sensitive",
      }),
    ).resolves.toMatchObject({ key: "gmail.email" });
    await expect(getAiSession("session-1")).resolves.toMatchObject({ sessionId: "session-1" });
  });

  it("wraps extension management commands", async () => {
    await expect(listExtensions()).resolves.toEqual([]);
    await expect(
      importExtensionFromStore({
        storeUrl: "https://chromewebstore.google.com/detail/name/cjpalhdlnbpafiamejdnhcphjbkeiagm",
        defaultEnabled: true,
        profileIds: ["profile-1"],
      }),
    ).resolves.toMatchObject({ source: "chromeStore", defaultEnabled: true });
    await expect(
      importExtensionFromLocal({
        localPath: "D:\\Downloads\\extension.crx",
        defaultEnabled: false,
        profileIds: [],
      }),
    ).resolves.toMatchObject({ source: "local", installPath: "D:\\Downloads\\extension.crx" });
    await expect(
      updateExtension("extension-2", {
        name: "Renamed",
        defaultEnabled: true,
        profileIds: ["profile-1"],
      }),
    ).resolves.toMatchObject({ name: "Renamed", profileIds: ["profile-1"] });
    await expect(deleteExtension("extension-2")).resolves.toBeUndefined();
  });

  it("wraps local API settings and key commands", async () => {
    await expect(getLocalApiSettings()).resolves.toEqual({
      enabled: false,
      port: 17688,
      bindMode: "loopback",
      updatedAt: null,
    });
    await expect(
      updateLocalApiSettings({ enabled: true, port: 17688, bindMode: "loopback" }),
    ).resolves.toMatchObject({ enabled: true, port: 17688 });
    await expect(
      updateLocalApiSettings({ enabled: true, port: 80, bindMode: "loopback" }),
    ).rejects.toMatchObject({ code: "localApi.invalidPort" });
    await expect(listLocalApiKeys()).resolves.toMatchObject([{ id: "key-1", suffix: "ABC123" }]);
    await expect(
      generateLocalApiKey("Default", {
        password: "correct horse battery staple",
        twoFactorCode: null,
      }),
    ).resolves.toMatchObject({
      key: { id: "key-2", suffix: "XYZ789" },
      fullKey: "veil_generated_XYZ789",
    });
    await expect(
      generateLocalApiKey("Default", { password: "wrong", twoFactorCode: null }),
    ).rejects.toMatchObject({ code: "security.verificationFailed" });
    await expect(
      rotateLocalApiKey("key-1", {
        password: "correct horse battery staple",
        twoFactorCode: null,
      }),
    ).resolves.toMatchObject({
      key: { id: "key-1", suffix: "ROTATE" },
      fullKey: "veil_rotated_ROTATE",
    });
  });
});
