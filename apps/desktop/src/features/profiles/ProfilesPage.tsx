import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import type {
  AppErrorCode,
  InstalledRuntimes,
  Profile,
  Proxy,
  RuntimeInstallState,
} from "@veil/shared";
import { emptyInstalledRuntimes, runtimeNeedsInstall } from "@veil/shared";
import { Button, ConfirmDialog, IconButton, Select } from "@veil/ui";
import {
  AppError,
  deleteProfile,
  launchProfile,
  listInstalledRuntimes,
  listProfiles,
  listProxies,
  stopProfile,
} from "../../api/client";
import { Icon } from "../../shell/Icon";
import { useAppStore } from "../../app/store";
import { BatchBar } from "./BatchBar";
import { BatchModifyProxyDialog } from "./BatchModifyProxyDialog";
import { EngineMissingBanner } from "./EngineMissingBanner";
import { ExportProfilesDialog } from "./ExportProfilesDialog";
import { ImportProfilesDialog } from "./ImportProfilesDialog";
import { buildProfilesEmptyState } from "./emptyState";
import { ProfileFormDialog } from "./ProfileFormDialog";
import { ProfilesCards } from "./ProfilesCards";
import { ProfilesTable } from "./ProfilesTable";
import { applyFilters, type FilterState, type SortKey, type StatusFilter } from "./filter";
import { buildLaunchDiagnostics } from "./launchDiagnostics";
import {
  applyProfileRuntimeOverlays,
  launchFailureCode,
  shouldPollProfileRuntime,
  type ProfileRuntimeOverlays,
} from "./runtimeState";
import { formatProxyOption } from "../proxy/format";

const DEFAULT_FILTERS: FilterState = {
  search: "",
  status: "all",
  sortKey: "name",
  sortDir: "asc",
};
const PROFILE_RUNTIME_POLL_MS = 3000;

/** How the profile list is laid out. The data flow is shared; only the rendering differs. */
type ProfileView = "table" | "cards";

export interface ProfilesPageProps {
  /**
   * Register an opener for the create dialog so the global plus button in `TopToolbar`
   * can open it without prop-drilling through the shell. Called once on mount.
   */
  onCreateProfileRegister?: (opener: () => void) => void;
  runtime: RuntimeInstallState | null;
  onOpenBrowserSection: () => void;
}

/** Discriminated union — clearer than two booleans for the dialog mode. */
type DialogState = { kind: "closed" } | { kind: "create" } | { kind: "edit"; profile: Profile };

type ConfirmState =
  { kind: "closed" } | { kind: "single"; profile: Profile } | { kind: "batch"; ids: string[] };

type ProxyDialogState = {
  profileIds: string[];
  profileName?: string;
  initialProxyId?: string | null;
  clearSelectionOnApply: boolean;
} | null;

/**
 * Profiles section: orchestrates loading, search/filter/sort, batch actions, and rendering.
 * Holds no business logic — everything that touches data goes through `api/client`.
 */
export function ProfilesPage({
  onCreateProfileRegister,
  runtime,
  onOpenBrowserSection,
}: ProfilesPageProps): ReactNode {
  const t = useT();
  const search = useAppStore((s) => s.search);
  const { selectedProfileIds, clearSelection, bumpProfiles } = useAppStore();
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [installedRuntimes, setInstalledRuntimes] =
    useState<InstalledRuntimes>(emptyInstalledRuntimes);
  const [loading, setLoading] = useState(true);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [sortKey, setSortKey] = useState<SortKey>(DEFAULT_FILTERS.sortKey);
  const [sortDir, setSortDir] = useState<"asc" | "desc">(DEFAULT_FILTERS.sortDir);
  const [dialog, setDialog] = useState<DialogState>({ kind: "closed" });
  const [confirm, setConfirm] = useState<ConfirmState>({ kind: "closed" });
  const [proxyDialog, setProxyDialog] = useState<ProxyDialogState>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [exportResult, setExportResult] = useState<{ count: number; path: string } | null>(null);
  const [view, setView] = useState<ProfileView>("table");
  const [runtimeOverlays, setRuntimeOverlays] = useState<ProfileRuntimeOverlays>({});

  const reload = useCallback(async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const [nextProfiles, nextProxies, nextRuntimes] = await Promise.all([
        listProfiles(),
        listProxies().catch(() => [] as Proxy[]),
        listInstalledRuntimes().catch(emptyInstalledRuntimes),
      ]);
      setProfiles(nextProfiles);
      setProxies(nextProxies);
      setInstalledRuntimes(nextRuntimes);
    } finally {
      if (showLoading) setLoading(false);
    }
  }, []);

  const proxyNames = useMemo(() => {
    const map = new Map<string, string>();
    for (const proxy of proxies) map.set(proxy.id, formatProxyOption(proxy));
    return map;
  }, [proxies]);

  const proxyById = useMemo(() => {
    const map = new Map<string, Proxy>();
    for (const proxy of proxies) map.set(proxy.id, proxy);
    return map;
  }, [proxies]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const openCreate = useCallback(() => setDialog({ kind: "create" }), []);

  useEffect(() => {
    onCreateProfileRegister?.(openCreate);
  }, [onCreateProfileRegister, openCreate]);

  const displayProfiles = useMemo(
    () => applyProfileRuntimeOverlays(profiles, runtimeOverlays),
    [profiles, runtimeOverlays],
  );
  const shouldPollRuntime = shouldPollProfileRuntime(displayProfiles);

  useEffect(() => {
    if (!shouldPollRuntime) return;
    const id = window.setInterval(() => {
      void reload(false);
    }, PROFILE_RUNTIME_POLL_MS);
    return () => window.clearInterval(id);
  }, [reload, shouldPollRuntime]);

  const launchErrors = useMemo(() => {
    const errors: Record<string, AppErrorCode | null> = {};
    for (const [profileId, overlay] of Object.entries(runtimeOverlays)) {
      if (overlay?.errorCode) errors[profileId] = overlay.errorCode;
    }
    return errors;
  }, [runtimeOverlays]);

  const effectiveInstalledRuntimes = useMemo<InstalledRuntimes>(() => {
    if (installedRuntimes.installed.length > 0 || runtime?.kind !== "installed") {
      return installedRuntimes;
    }
    return {
      ...installedRuntimes,
      currentVersion: runtime.current.version,
      installed: [runtime.current],
    };
  }, [installedRuntimes, runtime]);

  const diagnosticsByProfile = useMemo(() => {
    const diagnostics: Record<string, ReturnType<typeof buildLaunchDiagnostics>> = {};
    for (const profile of profiles) {
      diagnostics[profile.id] = buildLaunchDiagnostics(
        profile,
        effectiveInstalledRuntimes,
        profile.proxyId ? (proxyById.get(profile.proxyId) ?? null) : null,
      );
    }
    return diagnostics;
  }, [effectiveInstalledRuntimes, profiles, proxyById]);

  const filtered = useMemo(
    () => applyFilters(displayProfiles, { search, status: statusFilter, sortKey, sortDir }),
    [displayProfiles, search, statusFilter, sortKey, sortDir],
  );

  function toggleSort(next: SortKey): void {
    if (next === sortKey) {
      setSortDir((dir) => (dir === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(next);
      setSortDir("asc");
    }
  }

  async function refreshAfterMutation(): Promise<void> {
    await reload();
    bumpProfiles();
  }

  function setRuntimeOverlay(
    profileId: string,
    status: Profile["status"],
    errorCode: AppErrorCode | null,
  ): void {
    setRuntimeOverlays((prev) => ({
      ...prev,
      [profileId]: { status, errorCode },
    }));
  }

  function clearRuntimeOverlays(profileIds: string[]): void {
    setRuntimeOverlays((prev) => {
      const next = { ...prev };
      for (const profileId of profileIds) delete next[profileId];
      return next;
    });
  }

  function appErrorCode(err: unknown): AppErrorCode {
    return err instanceof AppError ? err.code : "app.unknown";
  }

  async function launchOne(profileId: string): Promise<boolean> {
    const preflightError =
      diagnosticsByProfile[profileId]?.preflightError ??
      (!runtime || runtimeNeedsInstall(runtime) ? "launch.runtimeMissing" : null);
    if (preflightError) {
      setRuntimeOverlay(profileId, "error", preflightError);
      return false;
    }
    setRuntimeOverlay(profileId, "launching", null);
    try {
      const result = await launchProfile(profileId);
      const code = launchFailureCode(result);
      if (code) {
        setRuntimeOverlay(profileId, "error", code);
        return false;
      }
      setRuntimeOverlay(profileId, "running", null);
      return true;
    } catch (err) {
      setRuntimeOverlay(profileId, "error", appErrorCode(err));
      return false;
    }
  }

  async function stopOne(profileId: string): Promise<boolean> {
    setRuntimeOverlay(profileId, "idle", null);
    try {
      await stopProfile(profileId);
      return true;
    } catch (err) {
      setRuntimeOverlay(profileId, "error", appErrorCode(err));
      return false;
    }
  }

  async function handleLaunch(profile: Profile): Promise<void> {
    const ok = await launchOne(profile.id);
    if (ok) {
      await refreshAfterMutation();
      clearRuntimeOverlays([profile.id]);
    }
  }

  async function handleStop(profile: Profile): Promise<void> {
    const ok = await stopOne(profile.id);
    if (ok) {
      await refreshAfterMutation();
      clearRuntimeOverlays([profile.id]);
    }
  }

  async function batchStart(): Promise<void> {
    const ids = Array.from(selectedProfileIds);
    const results = await Promise.all(ids.map((id) => launchOne(id)));
    clearSelection();
    await refreshAfterMutation();
    clearRuntimeOverlays(ids.filter((_, index) => results[index]));
  }

  async function batchStop(): Promise<void> {
    const ids = Array.from(selectedProfileIds);
    const results = await Promise.all(ids.map((id) => stopOne(id)));
    clearSelection();
    await refreshAfterMutation();
    clearRuntimeOverlays(ids.filter((_, index) => results[index]));
  }

  function askBatchDelete(): void {
    if (selectedProfileIds.size === 0) return;
    setConfirm({ kind: "batch", ids: Array.from(selectedProfileIds) });
  }

  async function confirmDeletion(): Promise<void> {
    if (confirm.kind === "single") {
      await deleteProfile(confirm.profile.id).catch(() => undefined);
    } else if (confirm.kind === "batch") {
      await Promise.all(confirm.ids.map((id) => deleteProfile(id).catch(() => undefined)));
      clearSelection();
    }
    setConfirm({ kind: "closed" });
    await refreshAfterMutation();
  }

  const selectedCount = selectedProfileIds.size;
  const hasProfiles = displayProfiles.length > 0;
  const emptyState = buildProfilesEmptyState(runtime);

  return (
    <section className="veil-section profiles-page">
      {runtime && runtimeNeedsInstall(runtime) ? (
        <EngineMissingBanner onOpenBrowser={onOpenBrowserSection} />
      ) : null}
      <header className="profiles-page__header">
        <div className="profiles-page__heading">
          <h2 className="veil-section__title">{t("profiles.title")}</h2>
          <p className="profiles-page__subtitle">{t("app.tagline")}</p>
        </div>
        <span className="veil-section__spacer" />
        {selectedCount > 0 ? (
          <BatchBar
            count={selectedCount}
            onStart={batchStart}
            onStop={batchStop}
            onDelete={askBatchDelete}
            onModifyProxy={() =>
              setProxyDialog({
                profileIds: Array.from(selectedProfileIds),
                clearSelectionOnApply: true,
              })
            }
            onExport={() => setExportOpen(true)}
            onClear={clearSelection}
          />
        ) : null}
        <Button variant="secondary" size="sm" onClick={() => setImportOpen(true)}>
          <Icon name="import" size={14} />
          {t("profiles.batch.import")}
        </Button>
        <Button variant="secondary" size="sm" onClick={() => void reload()}>
          <Icon name="refresh" size={14} />
          {t("toolbar.refresh")}
        </Button>
        <Button variant="primary" size="sm" onClick={openCreate}>
          <Icon name="plus" size={14} />
          {t("profiles.create")}
        </Button>
      </header>

      <div className="profiles-toolbar">
        <div className="profiles-toolbar__filters">
          <Select
            label={t("profiles.filter.status")}
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.currentTarget.value as StatusFilter)}
            options={[
              { value: "all", label: t("common.all") },
              { value: "idle", label: t("profiles.status.idle") },
              { value: "launching", label: t("profiles.status.launching") },
              { value: "running", label: t("profiles.status.running") },
              { value: "error", label: t("profiles.status.error") },
            ]}
          />
        </div>
        <span className="veil-section__spacer" />
        <span className="profiles-toolbar__count">
          {t("status.profileCount", { count: filtered.length })}
        </span>
        <div className="profiles-toolbar__view" role="group" aria-label={t("profiles.view.label")}>
          <IconButton
            label={t("profiles.view.table")}
            aria-pressed={view === "table"}
            data-active={view === "table"}
            onClick={() => setView("table")}
          >
            <Icon name="table" size={16} />
          </IconButton>
          <IconButton
            label={t("profiles.view.cards")}
            aria-pressed={view === "cards"}
            data-active={view === "cards"}
            onClick={() => setView("cards")}
          >
            <Icon name="cards" size={16} />
          </IconButton>
        </div>
      </div>

      {exportResult ? (
        <div className="profiles-export-result" role="status">
          {t("profiles.export.success", {
            count: exportResult.count,
            path: exportResult.path,
          })}
        </div>
      ) : null}

      {!hasProfiles && !loading ? (
        <div className="veil-empty">
          <span className="veil-empty__title">{t(emptyState.titleKey)}</span>
          <span>{t(emptyState.bodyKey)}</span>
          <Button
            variant="primary"
            onClick={emptyState.action === "openKernel" ? onOpenBrowserSection : openCreate}
          >
            <Icon name={emptyState.action === "openKernel" ? "browser" : "plus"} size={14} />
            {t(emptyState.actionKey)}
          </Button>
        </div>
      ) : view === "cards" ? (
        <ProfilesCards
          profiles={filtered}
          proxyNames={proxyNames}
          launchErrors={launchErrors}
          launchDiagnostics={diagnosticsByProfile}
          onLaunch={(p) => void handleLaunch(p)}
          onStop={(p) => void handleStop(p)}
          onEdit={(p) => setDialog({ kind: "edit", profile: p })}
          onModifyProxy={(p) =>
            setProxyDialog({
              profileIds: [p.id],
              profileName: p.name,
              initialProxyId: p.proxyId,
              clearSelectionOnApply: false,
            })
          }
          onDelete={(p) => setConfirm({ kind: "single", profile: p })}
        />
      ) : (
        <ProfilesTable
          profiles={filtered}
          proxyNames={proxyNames}
          launchErrors={launchErrors}
          launchDiagnostics={diagnosticsByProfile}
          sortKey={sortKey}
          sortDir={sortDir}
          onChangeSort={toggleSort}
          onLaunch={(p) => void handleLaunch(p)}
          onStop={(p) => void handleStop(p)}
          onEdit={(p) => setDialog({ kind: "edit", profile: p })}
          onModifyProxy={(p) =>
            setProxyDialog({
              profileIds: [p.id],
              profileName: p.name,
              initialProxyId: p.proxyId,
              clearSelectionOnApply: false,
            })
          }
          onDelete={(p) => setConfirm({ kind: "single", profile: p })}
        />
      )}

      <ProfileFormDialog
        open={dialog.kind !== "closed"}
        profile={dialog.kind === "edit" ? dialog.profile : undefined}
        onClose={() => setDialog({ kind: "closed" })}
        onSaved={() => {
          setDialog({ kind: "closed" });
          void refreshAfterMutation();
        }}
      />

      <ConfirmDialog
        open={confirm.kind !== "closed"}
        danger
        title={
          confirm.kind === "batch"
            ? t("profiles.confirm.batchDelete.title", { count: confirm.ids.length })
            : t("profiles.confirm.delete.title")
        }
        body={
          confirm.kind === "batch"
            ? t("profiles.confirm.batchDelete.body")
            : t("profiles.confirm.delete.body")
        }
        confirmLabel={t("common.delete")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void confirmDeletion()}
        onClose={() => setConfirm({ kind: "closed" })}
      />

      <BatchModifyProxyDialog
        open={Boolean(proxyDialog)}
        profileIds={proxyDialog?.profileIds ?? []}
        profileName={proxyDialog?.profileName}
        initialProxyId={proxyDialog?.initialProxyId}
        onClose={() => setProxyDialog(null)}
        onApplied={() => {
          const shouldClearSelection = proxyDialog?.clearSelectionOnApply ?? false;
          setProxyDialog(null);
          if (shouldClearSelection) clearSelection();
          void refreshAfterMutation();
        }}
      />

      <ExportProfilesDialog
        open={exportOpen}
        profileIds={Array.from(selectedProfileIds)}
        onClose={() => setExportOpen(false)}
        onExported={(result) => {
          setExportResult(result);
          clearSelection();
        }}
      />

      <ImportProfilesDialog
        open={importOpen}
        onClose={() => setImportOpen(false)}
        onImported={() => void refreshAfterMutation()}
      />
    </section>
  );
}
