import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import "../styles/profiles.css";
import "../styles/settings.css";
import "../styles/browser.css";
import "../styles/proxy.css";
import "../styles/ai.css";
import "../styles/extensions.css";
import "../styles/api.css";
import "../styles/logs.css";
import "../styles/onboarding.css";
import "../styles/security.css";
import type { RuntimeInstallState } from "@veil/shared";
import {
  getOnboardingStatus,
  getRuntimeState,
  getVaultStatus,
  isTauriEnvironment,
  listProfiles,
  type VaultStatus,
} from "../api/client";
import { AiPage } from "../features/ai/AiPage";
import { BrowserPage } from "../features/browser/BrowserPage";
import { EngineInstallDock } from "../features/browser/EngineInstallDock";
import { ExtensionsPage } from "../features/extensions/ExtensionsPage";
import { ApiPage } from "../features/api/ApiPage";
import { LogsPage } from "../features/logs/LogsPage";
import { OnboardingWizard } from "../features/onboarding/OnboardingWizard";
import { ProfilesPage } from "../features/profiles/ProfilesPage";
import { ProxyPage } from "../features/proxy/ProxyPage";
import { UnlockVaultScreen } from "../features/security/UnlockVaultScreen";
import { SettingsPage } from "../features/settings/SettingsPage";
import { Sidebar } from "../shell/Sidebar";
import { SectionTransition } from "../shell/SectionTransition";
import { StatusBar } from "../shell/StatusBar";
import { ToastHost } from "../shell/ToastHost";
import { TopToolbar } from "../shell/TopToolbar";
import { useEngineInstallStore } from "./engineInstallStore";
import { useAppStore } from "./store";

/** Top-level app: shell + the active section. View-only — all logic lives in services. */
export function App(): ReactNode {
  const { section, sidebarCollapsed, setSection, profilesVersion } = useAppStore();
  const [runtime, setRuntime] = useState<RuntimeInstallState | null>(null);
  const [profileCount, setProfileCount] = useState(0);
  const [onboardingCompleted, setOnboardingCompleted] = useState<boolean | null>(
    isTauriEnvironment() ? null : true,
  );
  const [vaultStatus, setVaultStatus] = useState<VaultStatus | null>(
    isTauriEnvironment() ? null : { locked: false, passwordRequired: false, wrapper: "dpapi" },
  );
  const createDialogRef = useRef<(() => void) | null>(null);

  const reloadRuntime = useCallback(() => {
    getRuntimeState()
      .then(setRuntime)
      .catch(() => setRuntime({ kind: "missing" }));
  }, []);

  // An install now runs from the shell, so it can finish while the user is on any section —
  // the status bar and the "no kernel" banner must not stay stale until they open Browser again.
  const installsCompleted = useEngineInstallStore((state) => state.completedCount);
  useEffect(() => {
    if (installsCompleted === 0) return;
    reloadRuntime();
  }, [installsCompleted, reloadRuntime]);

  useEffect(() => {
    if (!isTauriEnvironment()) return;
    let cancelled = false;
    Promise.all([getOnboardingStatus(), getVaultStatus()])
      .then(([onboarding, vault]) => {
        if (cancelled) return;
        setOnboardingCompleted(onboarding.completed);
        setVaultStatus(vault);
      })
      .catch(() => {
        if (cancelled) return;
        setOnboardingCompleted(true);
        setVaultStatus({ locked: true, passwordRequired: true, wrapper: "password" });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    reloadRuntime();
  }, [reloadRuntime, section]);

  useEffect(() => {
    if (onboardingCompleted !== true || vaultStatus?.locked) {
      setProfileCount(0);
      return;
    }
    listProfiles()
      .then((profiles) => setProfileCount(profiles.length))
      .catch(() => setProfileCount(0));
  }, [onboardingCompleted, section, profilesVersion, vaultStatus?.locked]);

  async function handleOnboardingComplete(): Promise<void> {
    setOnboardingCompleted(true);
    if (!isTauriEnvironment()) return;
    try {
      setVaultStatus(await getVaultStatus());
    } catch {
      setVaultStatus({ locked: true, passwordRequired: true, wrapper: "password" });
    }
  }

  function openCreateProfile(): void {
    // Make sure the profiles section is visible, then invoke the dialog opener that
    // ProfilesPage registered.
    setSection("profiles");
    createDialogRef.current?.();
  }

  if (onboardingCompleted === null) {
    return (
      <div className="veil-app">
        <div className="veil-app__loading">Veil</div>
      </div>
    );
  }

  if (!onboardingCompleted) {
    return <OnboardingWizard onComplete={() => void handleOnboardingComplete()} />;
  }

  if (vaultStatus === null) {
    return (
      <div className="veil-app">
        <div className="veil-app__loading">Veil</div>
      </div>
    );
  }

  if (vaultStatus.locked) {
    return <UnlockVaultScreen onUnlocked={setVaultStatus} />;
  }

  return (
    <div className="veil-app" data-sidebar={sidebarCollapsed ? "collapsed" : "expanded"}>
      <TopToolbar onCreateProfile={openCreateProfile} />
      <div className="veil-body" data-sidebar={sidebarCollapsed ? "collapsed" : "expanded"}>
        <Sidebar />
        <main className="veil-main" role="main">
          <SectionTransition section={section}>
            {(activeSection) =>
              activeSection === "profiles" ? (
                <ProfilesPage
                  runtime={runtime}
                  onCreateProfileRegister={(opener) => {
                    createDialogRef.current = opener;
                  }}
                  onOpenBrowserSection={() => setSection("kernel")}
                />
              ) : activeSection === "kernel" ? (
                <BrowserPage onRuntimeChanged={reloadRuntime} />
              ) : activeSection === "proxy" ? (
                <ProxyPage />
              ) : activeSection === "ai" ? (
                <AiPage />
              ) : activeSection === "extensions" ? (
                <ExtensionsPage />
              ) : activeSection === "api" ? (
                <ApiPage />
              ) : activeSection === "settings" ? (
                <SettingsPage onOnboardingReset={() => setOnboardingCompleted(false)} />
              ) : activeSection === "logs" ? (
                <LogsPage />
              ) : null
            }
          </SectionTransition>
        </main>
      </div>
      <StatusBar runtime={runtime} profileCount={profileCount} isOnline={navigator.onLine} />
      {/* Shell-level, so both survive leaving the Browser page mid-install. */}
      <EngineInstallDock />
      <ToastHost />
    </div>
  );
}
