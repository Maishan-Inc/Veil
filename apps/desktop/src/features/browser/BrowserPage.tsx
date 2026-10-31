import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useT } from "@veil/i18n";
import type {
  AvailableRuntime,
  AppErrorCode,
  InstalledRuntimes,
  RuntimeVersion,
} from "@veil/shared";
import { emptyInstalledRuntimes } from "@veil/shared";
import { Badge, Banner, Button, Modal, Spinner } from "@veil/ui";
import { checkAppUpdate, type AppUpdate } from "../../api/appUpdater";
import {
  AppError,
  listAvailableRuntimes,
  listInstalledRuntimes,
  restartApp,
  setCurrentRuntime,
  uninstallRuntimeVersion,
} from "../../api/client";
import { useEngineInstallStore, type RuntimeInstallTarget } from "../../app/engineInstallStore";
import { Icon } from "../../shell/Icon";
import { AvailableVersionRow } from "./AvailableVersionRow";
import { getCachedAvailableRuntimes, rememberAvailableRuntimes } from "./availableRuntimeCache";
import { CurrentVersionCard } from "./CurrentVersionCard";
import { formatBytes, formatTimestamp } from "./format";
import { buildInstalledVersionList } from "./installedVersions";
import { InstalledVersionRow } from "./InstalledVersionRow";
import { buildRuntimeReadiness } from "./runtimeReadiness";

type BrowserNotice =
  { kind: "loaded"; count: number } | { kind: "update"; count: number; version: string };

/**
 * The dialog only *starts* an install now. Progress, cancel, retry and the completion toast
 * belong to `EngineInstallDock` / `ToastHost` in the shell, because a ~2-minute download outlives
 * both this dialog and this page.
 */
type RuntimeInstallDialog = { kind: "closed" } | { kind: "confirm"; target: RuntimeInstallTarget };

interface BrowserPageProps {
  onRuntimeChanged?: () => void;
}

export function BrowserPage({ onRuntimeChanged }: BrowserPageProps): ReactNode {
  const t = useT();
  const [installed, setInstalled] = useState<InstalledRuntimes>(emptyInstalledRuntimes);
  const cachedAvailable = getCachedAvailableRuntimes();
  const [available, setAvailable] = useState<AvailableRuntime[]>(cachedAvailable ?? []);
  const [availableState, setAvailableState] = useState<"notLoaded" | "idle" | "loading" | "error">(
    cachedAvailable ? "idle" : "notLoaded",
  );
  const [availableError, setAvailableError] = useState<AppErrorCode | null>(null);
  const [localZipPath, setLocalZipPath] = useState("");
  const [notice, setNotice] = useState<BrowserNotice | null>(null);
  const [appUpdate, setAppUpdate] = useState<AppUpdate | null>(null);
  const [appUpdateState, setAppUpdateState] = useState<
    "idle" | "checking" | "current" | "available" | "installing" | "installed" | "error"
  >("idle");
  const [installDialog, setInstallDialog] = useState<RuntimeInstallDialog>({ kind: "closed" });
  const startInstall = useEngineInstallStore((state) => state.start);
  const installsCompleted = useEngineInstallStore((state) => state.completedCount);

  const reloadInstalled = useCallback(
    async (notifyParent = false) => {
      try {
        setInstalled(await listInstalledRuntimes());
      } catch {
        setInstalled(emptyInstalledRuntimes());
      } finally {
        if (notifyParent) onRuntimeChanged?.();
      }
    },
    [onRuntimeChanged],
  );

  const refreshAvailable = useCallback(
    async (currentVersion = installed.currentVersion) => {
      setAvailableState("loading");
      setAvailableError(null);
      try {
        const list = await listAvailableRuntimes();
        rememberAvailableRuntimes(list);
        setAvailable(list);
        const latest = list[0] ?? null;
        const hasUpdate =
          latest && currentVersion
            ? compareRuntimeVersions(latest.version, currentVersion) > 0
            : false;
        setNotice(
          hasUpdate && latest
            ? { kind: "update", count: list.length, version: latest.version }
            : { kind: "loaded", count: list.length },
        );
        setAvailableState("idle");
        return list;
      } catch (err) {
        const code = err instanceof AppError ? err.code : "app.unknown";
        const cached = getCachedAvailableRuntimes();
        if (cached) setAvailable(cached);
        setAvailableError(code);
        setNotice(null);
        setAvailableState("error");
        return cached ?? [];
      }
    },
    [installed.currentVersion],
  );

  useEffect(() => {
    let cancelled = false;

    async function initializeBrowserPage(): Promise<void> {
      try {
        const nextInstalled = await listInstalledRuntimes();
        if (!cancelled) setInstalled(nextInstalled);
      } catch {
        if (!cancelled) setInstalled(emptyInstalledRuntimes());
      }
    }

    void initializeBrowserPage();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (notice?.kind !== "loaded") return;
    const timeout = window.setTimeout(() => setNotice(null), 4200);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  // The dock owns the install, so the registry changed somewhere this page cannot see.
  useEffect(() => {
    if (installsCompleted === 0) return;
    void reloadInstalled(true);
  }, [installsCompleted, reloadInstalled]);

  const installedVersionSet = useMemo(
    () => new Set(installed.installed.map((r) => r.version)),
    [installed.installed],
  );

  const readiness = useMemo(
    () =>
      buildRuntimeReadiness({
        installed,
        availableState,
        availableCount: available.length,
        localZipPath,
      }),
    [available.length, availableState, installed, localZipPath],
  );

  function candidateForLocalImport(candidates: AvailableRuntime[]): AvailableRuntime | null {
    return (
      candidates.find((candidate) => !installedVersionSet.has(candidate.version)) ??
      candidates[0] ??
      null
    );
  }

  async function startDialogInstall(target: RuntimeInstallTarget): Promise<void> {
    // Close first: the dialog has done its job once the user commits, and the dock takes over.
    setInstallDialog({ kind: "closed" });
    await startInstall(target);
  }

  async function chooseLocalZip(): Promise<void> {
    const selected = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "ZIP", extensions: ["zip"] }],
    });
    if (typeof selected === "string") {
      setLocalZipPath(selected);
      const candidate = localImportCandidate ?? candidateForLocalImport(await refreshAvailable());
      if (candidate && !installedVersionSet.has(candidate.version)) {
        setInstallDialog({
          kind: "confirm",
          target: { source: "localZip", candidate, localZipPath: selected },
        });
      }
    }
  }

  async function checkForAppUpdate(): Promise<void> {
    setAppUpdate(null);
    setAppUpdateState("checking");
    try {
      const update = await checkAppUpdate();
      setAppUpdate(update);
      setAppUpdateState(update ? "available" : "current");
    } catch {
      setAppUpdateState("error");
    }
  }

  async function installAppUpdate(): Promise<void> {
    if (!appUpdate) return;
    setAppUpdateState("installing");
    try {
      await appUpdate.install();
      setAppUpdateState("installed");
    } catch {
      setAppUpdateState("error");
    }
  }

  async function makeDefault(version: string): Promise<void> {
    try {
      await setCurrentRuntime(version);
    } catch {
      /* swallow — UI surfaces via subsequent reload */
    }
    await reloadInstalled(true);
  }

  async function uninstall(version: string): Promise<void> {
    try {
      await uninstallRuntimeVersion(version);
    } catch {
      /* surfaced by Banner above the list */
    }
    await reloadInstalled(true);
  }

  const current: RuntimeVersion | null =
    installed.installed.find((r) => r.version === installed.currentVersion) ?? null;

  const installedVersionRows = buildInstalledVersionList(installed);
  const localImportCandidate =
    available.find((candidate) => !installedVersionSet.has(candidate.version)) ??
    available[0] ??
    null;
  const noticeCandidate =
    notice?.kind === "update"
      ? (available.find((candidate) => candidate.version === notice.version) ?? null)
      : null;

  function updateLatestRuntime(): void {
    if (notice?.kind !== "update") return;
    if (installedVersionSet.has(notice.version)) {
      void makeDefault(notice.version);
      return;
    }
    if (noticeCandidate) {
      setInstallDialog({
        kind: "confirm",
        target: { source: "remote", candidate: noticeCandidate },
      });
    }
  }

  function closeInstallDialog(): void {
    setInstallDialog({ kind: "closed" });
  }

  const dialogCandidate = installDialog.kind === "closed" ? null : installDialog.target.candidate;
  const dialogTarget = installDialog.kind === "closed" ? null : installDialog.target;

  return (
    <section className="veil-section browser-page">
      <header className="veil-section__header">
        <h2 className="veil-section__title">{t("browser.title")}</h2>
        <span className="veil-section__spacer" />
        <Button size="sm" variant="secondary" onClick={() => void reloadInstalled(true)}>
          <Icon name="refresh" size={14} />
          {t("toolbar.refresh")}
        </Button>
      </header>

      <p className="browser-subtitle">{t("browser.subtitle")}</p>

      {notice ? (
        <aside className="browser-toast" data-tone={notice.kind} role="status" aria-live="polite">
          <span className="browser-toast__icon">
            <Icon name={notice.kind === "update" ? "import" : "check"} size={16} />
          </span>
          <div className="browser-toast__copy">
            <strong>
              {notice.kind === "update"
                ? t("browser.toast.update.title")
                : t("browser.toast.loaded.title")}
            </strong>
            <span>
              {notice.kind === "update"
                ? t("browser.toast.update.body", { version: notice.version })
                : t("browser.toast.loaded.body", { count: notice.count })}
            </span>
          </div>
          {notice.kind === "update" ? (
            <Button size="sm" variant="primary" onClick={updateLatestRuntime}>
              {t("browser.toast.update.action")}
            </Button>
          ) : null}
          <Button size="sm" variant="subtle" onClick={() => setNotice(null)}>
            {t("common.close")}
          </Button>
        </aside>
      ) : null}

      <div className="browser-dashboard">
        <CurrentVersionCard current={current} />
        <section className="browser-readiness" aria-label={t("browser.readiness.title")}>
          <div className="browser-readiness__head">
            <h3 className="browser-readiness__title">{t("browser.readiness.title")}</h3>
            <p className="browser-readiness__body">{t("browser.readiness.body")}</p>
          </div>
          <div className="browser-readiness__steps">
            {readiness.map((step) => (
              <article key={step.id} className="browser-readiness__step">
                <span className="browser-readiness__icon" data-tone={step.tone}>
                  <Icon name={readinessIcon(step.id)} size={16} />
                </span>
                <div className="browser-readiness__copy">
                  <span className="browser-readiness__step-title">{t(step.titleKey)}</span>
                  <span className="browser-readiness__detail">
                    {t(step.detailKey, step.params)}
                  </span>
                </div>
                <Badge tone={step.tone} dot>
                  {t(step.statusKey)}
                </Badge>
              </article>
            ))}
          </div>
        </section>
      </div>

      <section className="browser-section browser-actions-panel">
        <div className="browser-actions-panel__copy">
          <h3 className="browser-section__title">{t("browser.actions.title")}</h3>
          <p className="browser-actions-panel__body">{t("browser.actions.body")}</p>
        </div>
        <div className="browser-actions-panel__controls">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => void refreshAvailable()}
            disabled={availableState === "loading"}
            aria-label={availableState === "loading" ? t("browser.available.loading") : undefined}
          >
            {availableState === "loading" ? (
              <Spinner size={14} label={t("browser.available.loading")} />
            ) : (
              <>
                <Icon name="refresh" size={14} />
                {t("browser.action.loadHistory")}
              </>
            )}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => void chooseLocalZip()}>
            <Icon name="folder" size={14} />
            {t("browser.localZip.choose")}
          </Button>
        </div>
      </section>

      <section className="browser-section browser-app-update">
        <div>
          <h3 className="browser-section__title">{t("browser.appUpdate.title")}</h3>
          <p className="browser-actions-panel__body">{t("browser.appUpdate.body")}</p>
          {appUpdateState !== "idle" && appUpdateState !== "checking" ? (
            <p className="browser-app-update__status" role="status">
              {appUpdateState === "available" && appUpdate
                ? t("browser.appUpdate.available", { version: appUpdate.version })
                : t(`browser.appUpdate.${appUpdateState}`)}
            </p>
          ) : null}
        </div>
        <div className="browser-actions-panel__controls">
          {appUpdateState === "available" && appUpdate ? (
            <Button size="sm" variant="primary" onClick={() => void installAppUpdate()}>
              {t("browser.appUpdate.install")}
            </Button>
          ) : null}
          {appUpdateState === "installed" ? (
            <Button size="sm" variant="secondary" onClick={() => void restartApp()}>
              {t("browser.appUpdate.restart")}
            </Button>
          ) : null}
          <Button
            size="sm"
            variant="secondary"
            onClick={() => void checkForAppUpdate()}
            disabled={appUpdateState === "checking" || appUpdateState === "installing"}
          >
            {appUpdateState === "checking" ? (
              <Spinner size={14} label={t("browser.appUpdate.checking")} />
            ) : (
              t("browser.appUpdate.check")
            )}
          </Button>
        </div>
      </section>

      <div className="browser-main-grid">
        <section className="browser-section">
          <h3 className="browser-section__title">{t("browser.available.title")}</h3>
          {availableState === "loading" ? (
            available.length === 0 ? (
              <div className="browser-empty browser-empty--loading">
                <Spinner size={18} label={t("browser.available.loading")} />
              </div>
            ) : (
              <div className="browser-history-state" data-tone="info">
                <Spinner size={14} label={t("browser.available.loading")} />
              </div>
            )
          ) : null}
          {availableState === "error" && availableError ? (
            <Banner
              tone="warning"
              title={t(`errors.${availableError}`)}
              action={
                <Button size="sm" variant="secondary" onClick={() => void refreshAvailable()}>
                  {t("browser.action.retry")}
                </Button>
              }
            >
              {t("browser.available.errorBody")}
            </Banner>
          ) : null}
          {availableState === "idle" && available.length === 0 ? (
            <div className="browser-empty">{t("browser.available.empty")}</div>
          ) : null}
          {available.map((candidate) => (
            <AvailableVersionRow
              key={candidate.version}
              candidate={candidate}
              alreadyInstalled={installedVersionSet.has(candidate.version)}
              onInstall={() =>
                setInstallDialog({
                  kind: "confirm",
                  target: { source: "remote", candidate },
                })
              }
            />
          ))}
        </section>

        <section className="browser-section">
          <h3 className="browser-section__title">{t("browser.installed.title")}</h3>
          {installedVersionRows.length === 0 ? (
            <div className="browser-empty">{t("browser.installed.empty")}</div>
          ) : null}
          {installedVersionRows.map(({ runtime, isCurrent }) => (
            <InstalledVersionRow
              key={runtime.version}
              runtime={runtime}
              isCurrent={isCurrent}
              onSetDefault={() => void makeDefault(runtime.version)}
              onUninstall={() => void uninstall(runtime.version)}
            />
          ))}
        </section>
      </div>

      <Modal
        open={installDialog.kind !== "closed"}
        title={
          dialogTarget?.source === "localZip"
            ? t("browser.install.localZipTitle", {
                version: dialogTarget.candidate.version,
              })
            : dialogCandidate
              ? t("browser.install.title", { version: dialogCandidate.version })
              : t("browser.action.install")
        }
        onClose={closeInstallDialog}
        size="md"
        footer={
          installDialog.kind === "confirm" ? (
            <>
              <Button variant="subtle" onClick={closeInstallDialog}>
                {t("common.cancel")}
              </Button>
              <Button
                variant="primary"
                onClick={() => void startDialogInstall(installDialog.target)}
              >
                <Icon
                  name={installDialog.target.source === "localZip" ? "folder" : "import"}
                  size={14}
                />
                {installDialog.target.source === "localZip"
                  ? t("browser.install.localZipAction")
                  : t("browser.install.downloadInstall")}
              </Button>
            </>
          ) : null
        }
      >
        {dialogCandidate ? (
          <div className="browser-install-dialog">
            {dialogTarget?.source === "localZip" ? (
              <p className="browser-install-dialog__body">{t("browser.install.localZipBody")}</p>
            ) : (
              <p className="browser-install-dialog__body">{t("browser.install.body")}</p>
            )}
            <dl className="browser-install-dialog__meta">
              <div>
                <dt>{t("browser.versionSelect.label")}</dt>
                <dd>{dialogCandidate.version}</dd>
              </div>
              <div>
                <dt>{t("browser.meta.asset")}</dt>
                <dd>{dialogCandidate.assetName}</dd>
              </div>
              {dialogTarget?.source === "localZip" ? (
                <div className="browser-install-dialog__meta-wide">
                  <dt>{t("browser.localZip.path")}</dt>
                  <dd>{dialogTarget.localZipPath}</dd>
                </div>
              ) : null}
              <div>
                <dt>{t("browser.meta.size")}</dt>
                <dd>{formatBytes(dialogCandidate.sizeBytes)}</dd>
              </div>
              <div>
                <dt>{t("browser.meta.publishedAt")}</dt>
                <dd>
                  {dialogCandidate.publishedAt
                    ? formatTimestamp(dialogCandidate.publishedAt)
                    : t("common.none")}
                </dd>
              </div>
              <div className="browser-install-dialog__meta-wide">
                <dt>{t("browser.meta.checksum")}</dt>
                <dd>{dialogCandidate.sha256}</dd>
              </div>
            </dl>
          </div>
        ) : null}
      </Modal>
    </section>
  );
}

function readinessIcon(id: "kernel" | "history" | "localZip") {
  switch (id) {
    case "kernel":
    case "history":
      return "browser";
    case "localZip":
      return "folder";
  }
}

function compareRuntimeVersions(left: string, right: string): number {
  const leftParts = versionParts(left);
  const rightParts = versionParts(right);
  const length = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < length; index += 1) {
    const diff = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (diff !== 0) return diff;
  }
  return left.localeCompare(right);
}

function versionParts(version: string): number[] {
  return Array.from(version.matchAll(/\d+/g), (match) => Number(match[0]));
}
