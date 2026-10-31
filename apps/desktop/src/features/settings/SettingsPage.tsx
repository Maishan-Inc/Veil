import { useEffect, useState } from "react";
import { QRCodeSVG } from "@rc-component/qrcode";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useT } from "@veil/i18n";
import type { AiSettings, DataLocationInfo, EngineRootInfo } from "@veil/shared";
import { locales, type Locale } from "@veil/i18n";
import { Button, ConfirmDialog, Modal, Select, Spinner, TextField } from "@veil/ui";
import {
  getAiSettings,
  AppError,
  beginTotpEnrollment,
  cancelDataLocationMove,
  confirmTotpEnrollment,
  disableTotp,
  exportDiagnostics,
  getDataLocation,
  getOnboardingStatus,
  getRuntimeInstallRoot,
  getVaultStatus,
  isTauriEnvironment,
  previewDiagnostics,
  resetOnboarding,
  restartApp,
  setDataLocation,
  setRuntimeInstallRoot,
  updateAiSettings,
  type DiagnosticsBundle,
  type OnboardingStatus,
  type SensitiveActionInput,
  type TotpEnrollment,
  type VaultStatus,
} from "../../api/client";
import { useAppStore } from "../../app/store";
import { useToastStore } from "../../app/toastStore";
import { SensitiveActionDialog } from "../security/SensitiveActionDialog";

/** Render a byte count as whole-number gibibytes, or an em dash when unknown. */
function formatMemoryBytes(bytes: number | null): string {
  if (bytes === null) return "—";
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

/* ----------------------------- Settings page ----------------------------- */

interface SettingsPageProps {
  onOnboardingReset?: () => void;
}

export function SettingsPage({ onOnboardingReset }: SettingsPageProps) {
  const t = useT();
  const { themePreference, setThemePreference, locale, setLocale, setSection } = useAppStore();
  const [securityStatus, setSecurityStatus] = useState<OnboardingStatus | null>(null);
  const [vaultStatus, setVaultStatus] = useState<VaultStatus | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [securityError, setSecurityError] = useState<string | null>(null);
  const [totpAction, setTotpAction] = useState<"enable" | "disable" | null>(null);
  const [totpEnrollment, setTotpEnrollment] = useState<TotpEnrollment | null>(null);
  const [totpVerification, setTotpVerification] = useState<SensitiveActionInput | null>(null);
  const [totpCode, setTotpCode] = useState("");
  const [totpBusy, setTotpBusy] = useState(false);
  const [totpError, setTotpError] = useState<string | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);
  const [aiApiKey, setAiApiKey] = useState("");
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [runtimeInstallRoot, setRuntimeInstallRootState] = useState<EngineRootInfo | null>(null);
  const [movingRuntime, setMovingRuntime] = useState(false);
  const [runtimeMoveError, setRuntimeMoveError] = useState<string | null>(null);
  const [dataLocation, setDataLocationState] = useState<DataLocationInfo | null>(null);
  const [dataMoveError, setDataMoveError] = useState<string | null>(null);
  const [diagnosticsPreview, setDiagnosticsPreview] = useState<DiagnosticsBundle | null>(null);
  const [includeGraphicsDetails, setIncludeGraphicsDetails] = useState(false);
  const [diagnosticsBusy, setDiagnosticsBusy] = useState(false);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const pushToast = useToastStore((state) => state.push);

  useEffect(() => {
    if (!isTauriEnvironment()) return;
    void refreshSecurity();
    void getAiSettings()
      .then(setAiSettings)
      .catch(() => setAiSettings(null));
    void getRuntimeInstallRoot()
      .then(setRuntimeInstallRootState)
      .catch(() => setRuntimeInstallRootState(null));
    void getDataLocation()
      .then(setDataLocationState)
      .catch(() => setDataLocationState(null));
  }, []);

  /**
   * Queue a data-directory move. Unlike the engine root, this cannot happen now: the database and
   * every open profile directory live here, so the move runs at the next startup and the card
   * offers a restart.
   */
  async function handleChangeDataLocation(): Promise<void> {
    const selected = await openDialog({ multiple: false, directory: true });
    if (typeof selected !== "string") return;
    setDataMoveError(null);
    try {
      setDataLocationState(await setDataLocation(selected));
    } catch (err) {
      const code = err instanceof AppError ? err.code : "migration.failed";
      setDataMoveError(`errors.${code}`);
    }
  }

  async function handleCancelDataMove(): Promise<void> {
    setDataMoveError(null);
    try {
      setDataLocationState(await cancelDataLocationMove());
    } catch (err) {
      const code = err instanceof AppError ? err.code : "migration.failed";
      setDataMoveError(`errors.${code}`);
    }
  }

  /**
   * Pick a new engine directory and move what is already installed into it. The move happens in
   * Rust; this only keeps a busy affordance up, because a cross-volume move copies ~200 MB per
   * installed version and a silent several-second pause reads as a frozen window.
   */
  async function handleChangeRuntimeRoot(): Promise<void> {
    const selected = await openDialog({ multiple: false, directory: true });
    if (typeof selected !== "string") return;
    setMovingRuntime(true);
    setRuntimeMoveError(null);
    try {
      const moved = await setRuntimeInstallRoot(selected);
      setRuntimeInstallRootState(moved);
      pushToast({
        tone: "success",
        titleKey: "settings.runtime.moved.title",
        bodyKey: "settings.runtime.moved.body",
        params: { path: moved.path },
        ttlMs: 7000,
      });
    } catch (err) {
      const code = err instanceof AppError ? err.code : "migration.failed";
      setRuntimeMoveError(`errors.${code}`);
    } finally {
      setMovingRuntime(false);
    }
  }

  async function refreshSecurity(): Promise<void> {
    try {
      const [security, vault] = await Promise.all([getOnboardingStatus(), getVaultStatus()]);
      setSecurityStatus(security);
      setVaultStatus(vault);
    } catch {
      setSecurityStatus(null);
      setVaultStatus(null);
    }
  }

  async function handleResetOnboarding(): Promise<void> {
    setSecurityError(null);
    try {
      const status = await resetOnboarding();
      setSecurityStatus(status);
      setConfirmReset(false);
      onOnboardingReset?.();
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setSecurityError(`errors.${code}`);
    }
  }

  async function handleBeginTotp(verification: SensitiveActionInput): Promise<void> {
    setTotpError(null);
    const enrollment = await beginTotpEnrollment(verification);
    setTotpVerification(verification);
    setTotpEnrollment(enrollment);
    setTotpCode("");
  }

  async function handleConfirmTotp(): Promise<void> {
    if (!totpEnrollment || !totpVerification) return;
    setTotpBusy(true);
    setTotpError(null);
    try {
      const status = await confirmTotpEnrollment(totpEnrollment.secret, totpCode, totpVerification);
      setSecurityStatus(status);
      setTotpEnrollment(null);
      setTotpVerification(null);
      setTotpCode("");
    } catch (err) {
      const code = err instanceof AppError ? err.code : "security.verificationFailed";
      setTotpError(`errors.${code}`);
    } finally {
      setTotpBusy(false);
    }
  }

  async function handleDisableTotp(verification: SensitiveActionInput): Promise<void> {
    const status = await disableTotp(verification);
    setSecurityStatus(status);
  }

  async function handleSaveAiSettings(apiKey: string | null = null): Promise<void> {
    if (!aiSettings) return;
    setAiBusy(true);
    setAiError(null);
    try {
      const next = await updateAiSettings({
        providerKind: aiSettings.providerKind,
        baseUrl: aiSettings.baseUrl,
        model: aiSettings.model,
        reasoningEffort: aiSettings.reasoningEffort,
        reviewMode: aiSettings.reviewMode,
        networkEnabled: aiSettings.networkEnabled,
        apiKey,
      });
      setAiSettings(next);
      setAiApiKey("");
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setAiError(`errors.${code}`);
    } finally {
      setAiBusy(false);
    }
  }

  async function handlePreviewDiagnostics(): Promise<void> {
    setDiagnosticsError(null);
    setDiagnosticsBusy(true);
    try {
      setDiagnosticsPreview(await previewDiagnostics());
    } catch (err) {
      const code = err instanceof AppError ? err.code : "diagnostics.unavailable";
      setDiagnosticsError(`errors.${code}`);
    } finally {
      setDiagnosticsBusy(false);
    }
  }

  async function handleExportDiagnostics(): Promise<void> {
    const path = await saveDialog({
      defaultPath: "veil-diagnostics.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    setDiagnosticsError(null);
    setDiagnosticsBusy(true);
    try {
      const result = await exportDiagnostics({ path, includeGraphicsDetails });
      pushToast({
        tone: "success",
        titleKey: "settings.diagnostics",
        bodyKey: "settings.diagnostics.exported",
        params: { path: result.path },
        ttlMs: 7000,
      });
    } catch (err) {
      const code = err instanceof AppError ? err.code : "diagnostics.exportFailed";
      setDiagnosticsError(`errors.${code}`);
    } finally {
      setDiagnosticsBusy(false);
    }
  }

  return (
    <section className="veil-section">
      <header className="veil-section__header">
        <h2 className="veil-section__title">{t("settings.title")}</h2>
      </header>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.appearance")}</h3>
        <div className="settings-row">
          <Select
            label={t("settings.theme")}
            value={themePreference}
            onChange={(event) =>
              setThemePreference(event.currentTarget.value as "light" | "dark" | "auto")
            }
            options={[
              { value: "light", label: t("settings.theme.light") },
              { value: "dark", label: t("settings.theme.dark") },
              { value: "auto", label: t("settings.theme.auto") },
            ]}
          />
          <Select
            label={t("settings.language")}
            value={locale}
            onChange={(event) => setLocale(event.currentTarget.value as Locale)}
            options={locales.map((value) => ({
              value,
              label: t(`settings.language.${value}`),
            }))}
          />
        </div>
      </section>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.runtime")}</h3>
        <p className="settings-card__body">{t("browser.subtitle")}</p>
        <div className="settings-path">
          <span className="settings-path__label">{t("settings.runtime.installLocation")}</span>
          <code>
            {runtimeInstallRoot?.path ?? t("settings.runtime.installLocationUnavailable")}
          </code>
          {runtimeInstallRoot ? (
            <span className="settings-path__reason">
              {t(`settings.runtime.installSource.${runtimeInstallRoot.source}`)}
            </span>
          ) : null}
        </div>
        <div className="settings-card__actions">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void handleChangeRuntimeRoot()}
            disabled={movingRuntime}
          >
            {movingRuntime ? (
              <Spinner size={14} label={t("settings.runtime.moving")} />
            ) : (
              t("settings.runtime.changeLocation")
            )}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => setSection("kernel")}>
            {t("profiles.banner.engineMissing.action")}
          </Button>
        </div>
        {runtimeMoveError ? <p className="settings-card__error">{t(runtimeMoveError)}</p> : null}
      </section>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.data")}</h3>
        <p className="settings-card__body">{t("settings.data.body")}</p>
        <div className="settings-path">
          <span className="settings-path__label">{t("settings.data.location")}</span>
          <code>{dataLocation?.path ?? t("settings.runtime.installLocationUnavailable")}</code>
          {dataLocation?.isDefault ? (
            <span className="settings-path__reason">{t("settings.data.isDefault")}</span>
          ) : null}
        </div>
        {dataLocation?.pendingPath ? (
          <div className="settings-path" data-tone="pending">
            <span className="settings-path__label">{t("settings.data.pending")}</span>
            <code>{dataLocation.pendingPath}</code>
            <span className="settings-path__reason">{t("settings.data.pendingBody")}</span>
          </div>
        ) : null}
        <div className="settings-card__actions">
          {dataLocation?.pendingPath ? (
            <>
              <Button variant="primary" size="sm" onClick={() => void restartApp()}>
                {t("settings.data.restartNow")}
              </Button>
              <Button variant="subtle" size="sm" onClick={() => void handleCancelDataMove()}>
                {t("common.cancel")}
              </Button>
            </>
          ) : (
            <Button variant="secondary" size="sm" onClick={() => void handleChangeDataLocation()}>
              {t("settings.data.changeLocation")}
            </Button>
          )}
        </div>
        {dataMoveError ? <p className="settings-card__error">{t(dataMoveError)}</p> : null}
      </section>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.ai")}</h3>
        <p className="settings-card__body">{t("settings.ai.body")}</p>
        {aiSettings ? (
          <div className="settings-ai">
            <div className="settings-row">
              <Select
                label={t("settings.ai.provider")}
                value={aiSettings.providerKind}
                onChange={(event) =>
                  setAiSettings({ ...aiSettings, providerKind: event.currentTarget.value as never })
                }
                options={[
                  { value: "openai", label: t("settings.ai.provider.openai") },
                  {
                    value: "openaiCompatible",
                    label: t("settings.ai.provider.openaiCompatible"),
                  },
                ]}
              />
              <TextField
                label={t("settings.ai.baseUrl")}
                value={aiSettings.baseUrl}
                onChange={(event) =>
                  setAiSettings({ ...aiSettings, baseUrl: event.currentTarget.value })
                }
              />
              <TextField
                label={t("settings.ai.model")}
                value={aiSettings.model}
                onChange={(event) =>
                  setAiSettings({ ...aiSettings, model: event.currentTarget.value })
                }
              />
            </div>
            <div className="settings-row">
              <Select
                label={t("settings.ai.reasoning")}
                value={aiSettings.reasoningEffort}
                onChange={(event) =>
                  setAiSettings({
                    ...aiSettings,
                    reasoningEffort: event.currentTarget.value as never,
                  })
                }
                options={[
                  { value: "low", label: t("settings.ai.reasoning.low") },
                  { value: "medium", label: t("settings.ai.reasoning.medium") },
                  { value: "high", label: t("settings.ai.reasoning.high") },
                ]}
              />
              <Select
                label={t("settings.ai.reviewMode")}
                value={aiSettings.reviewMode}
                onChange={(event) =>
                  setAiSettings({ ...aiSettings, reviewMode: event.currentTarget.value as never })
                }
                options={[
                  { value: "reviewed", label: t("ai.mode.reviewed") },
                  { value: "fullAuto", label: t("ai.mode.fullAuto") },
                ]}
              />
              <TextField
                type="password"
                label={t("settings.ai.apiKey")}
                placeholder={
                  aiSettings.hasApiKey
                    ? t("settings.ai.apiKey.placeholder.saved")
                    : t("settings.ai.apiKey.placeholder.empty")
                }
                value={aiApiKey}
                onChange={(event) => setAiApiKey(event.currentTarget.value)}
              />
            </div>
            <label className="settings-check">
              <input
                type="checkbox"
                checked={aiSettings.networkEnabled}
                onChange={(event) =>
                  setAiSettings({ ...aiSettings, networkEnabled: event.currentTarget.checked })
                }
              />
              <span>{t("settings.ai.networkEnabled")}</span>
            </label>
            <p className="settings-card__body settings-card__note">
              {t("settings.ai.fullAutoNote")}
            </p>
            {aiError ? <p className="settings-card__error">{t(aiError)}</p> : null}
            <div className="settings-card__actions">
              <Button
                variant="primary"
                size="sm"
                disabled={aiBusy}
                onClick={() => void handleSaveAiSettings(aiApiKey.trim() ? aiApiKey : null)}
                aria-label={aiBusy ? t("common.loading") : undefined}
              >
                {aiBusy ? <Spinner size={14} label={t("common.loading")} /> : t("common.save")}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                disabled={aiBusy || !aiSettings.hasApiKey}
                onClick={() => void handleSaveAiSettings("")}
              >
                {t("settings.ai.clearApiKey")}
              </Button>
            </div>
          </div>
        ) : (
          <p className="settings-card__body">{t("settings.ai.unavailable")}</p>
        )}
      </section>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.security")}</h3>
        <p className="settings-card__body">{t("settings.security.body")}</p>
        {securityStatus ? (
          <dl className="settings-kv">
            <dt>{t("settings.security.vault")}</dt>
            <dd>
              {vaultStatus ? t(`settings.security.vault.${vaultStatus.wrapper}`) : t("common.none")}
            </dd>
            <dt>{t("settings.security.vaultStatus")}</dt>
            <dd>{vaultStatus?.locked ? t("status.locked") : t("status.ready")}</dd>
            <dt>{t("settings.security.mode")}</dt>
            <dd>{t(`settings.security.mode.${securityStatus.encryptionMode}`)}</dd>
            <dt>{t("settings.security.passwordRequired")}</dt>
            <dd>{securityStatus.passwordRequired ? t("common.yes") : t("common.no")}</dd>
            <dt>{t("settings.security.twoFactor")}</dt>
            <dd>
              {securityStatus.twoFactorEnabled
                ? t("settings.security.twoFactor.enabled")
                : securityStatus.twoFactorPending
                  ? t("settings.security.twoFactor.pending")
                  : t("common.no")}
            </dd>
            <dt>{t("settings.security.mnemonic")}</dt>
            <dd>{securityStatus.mnemonicEnabled ? t("common.yes") : t("common.no")}</dd>
            <dt>{t("settings.security.developer")}</dt>
            <dd>{securityStatus.developer}</dd>
            <dt>{t("settings.security.acceptedAt")}</dt>
            <dd>
              {securityStatus.agreementAcceptedAt
                ? new Date(securityStatus.agreementAcceptedAt).toLocaleString()
                : t("common.none")}
            </dd>
          </dl>
        ) : (
          <p className="settings-card__body">{t("settings.security.unavailable")}</p>
        )}
        <p className="settings-card__body settings-card__note">
          {t("settings.security.boundaryNote")}
        </p>
        {securityError ? <p className="settings-card__error">{t(securityError)}</p> : null}
        <div className="settings-card__actions">
          {securityStatus?.twoFactorEnabled ? (
            <Button variant="secondary" size="sm" onClick={() => setTotpAction("disable")}>
              {t("settings.security.2fa.disable")}
            </Button>
          ) : (
            <Button variant="secondary" size="sm" onClick={() => setTotpAction("enable")}>
              {t("settings.security.2fa.enable")}
            </Button>
          )}
          <Button variant="secondary" size="sm" onClick={() => setConfirmReset(true)}>
            {t("settings.security.resetOnboarding")}
          </Button>
        </div>
      </section>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.diagnostics")}</h3>
        <p className="settings-card__body">{t("settings.diagnostics.body")}</p>
        <p className="settings-card__body settings-card__note">
          {t("settings.diagnostics.systemNote")}
        </p>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={includeGraphicsDetails}
            onChange={(event) => setIncludeGraphicsDetails(event.currentTarget.checked)}
          />
          <span>{t("settings.diagnostics.graphics.optIn")}</span>
        </label>
        <p className="settings-card__body settings-card__note">
          {t("settings.diagnostics.graphics.note")}
        </p>
        <p className="settings-card__body settings-card__note">
          {t("settings.diagnostics.deleteAfterUse")}
        </p>
        {diagnosticsError ? <p className="settings-card__error">{t(diagnosticsError)}</p> : null}
        <div className="settings-card__actions">
          <Button
            variant="secondary"
            size="sm"
            disabled={diagnosticsBusy}
            onClick={() => void handlePreviewDiagnostics()}
          >
            {t("settings.diagnostics.preview")}
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={diagnosticsBusy}
            onClick={() => void handleExportDiagnostics()}
            aria-label={diagnosticsBusy ? t("common.loading") : undefined}
          >
            {diagnosticsBusy ? (
              <Spinner size={14} label={t("common.loading")} />
            ) : (
              t("settings.diagnostics.export")
            )}
          </Button>
        </div>
      </section>

      <section className="settings-card">
        <h3 className="settings-card__title">{t("settings.about")}</h3>
        <p className="settings-card__body">{t("settings.about.body")}</p>
      </section>

      <ConfirmDialog
        open={confirmReset}
        danger
        title={t("settings.security.resetOnboarding.title")}
        body={t("settings.security.resetOnboarding.body")}
        confirmLabel={t("settings.security.resetOnboarding")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void handleResetOnboarding()}
        onClose={() => setConfirmReset(false)}
      />

      <SensitiveActionDialog
        open={totpAction === "enable"}
        title={t("settings.security.2fa.enable")}
        body={t("settings.security.2fa.enableBody")}
        confirmLabel={t("common.continue")}
        onClose={() => setTotpAction(null)}
        onConfirm={handleBeginTotp}
      />

      <SensitiveActionDialog
        open={totpAction === "disable"}
        title={t("settings.security.2fa.disable")}
        body={t("settings.security.2fa.disableBody")}
        confirmLabel={t("settings.security.2fa.disable")}
        onClose={() => setTotpAction(null)}
        onConfirm={handleDisableTotp}
      />

      <Modal
        open={totpEnrollment !== null}
        title={t("settings.security.2fa.confirmTitle")}
        onClose={() => {
          if (totpBusy) return;
          setTotpEnrollment(null);
          setTotpVerification(null);
          setTotpCode("");
        }}
        size="md"
        footer={
          <>
            <Button
              variant="subtle"
              disabled={totpBusy}
              onClick={() => {
                setTotpEnrollment(null);
                setTotpVerification(null);
                setTotpCode("");
              }}
            >
              {t("common.cancel")}
            </Button>
            <Button
              variant="primary"
              disabled={totpBusy || totpCode.trim().length === 0}
              onClick={() => void handleConfirmTotp()}
              aria-label={totpBusy ? t("common.loading") : undefined}
            >
              {totpBusy ? <Spinner size={16} label={t("common.loading")} /> : t("common.confirm")}
            </Button>
          </>
        }
      >
        <div className="settings-2fa">
          <p className="settings-card__body">{t("settings.security.2fa.confirmBody")}</p>
          {totpEnrollment ? (
            <div className="settings-2fa__qr" aria-label={t("settings.security.2fa.confirmTitle")}>
              <QRCodeSVG value={totpEnrollment.otpauthUri} size={184} marginSize={2} />
            </div>
          ) : null}
          <TextField
            label={t("settings.security.2fa.secret")}
            value={totpEnrollment?.secret ?? ""}
            readOnly
          />
          <TextField
            inputMode="numeric"
            label={t("security.verify.2fa")}
            placeholder={t("security.verify.2fa.placeholder")}
            value={totpCode}
            onChange={(event) => setTotpCode(event.currentTarget.value)}
            autoComplete="one-time-code"
          />
          {totpError ? <p className="settings-card__error">{t(totpError)}</p> : null}
        </div>
      </Modal>

      <Modal
        open={diagnosticsPreview !== null}
        title={t("diagnostics.preview.title")}
        onClose={() => setDiagnosticsPreview(null)}
        size="md"
        footer={
          <Button variant="subtle" onClick={() => setDiagnosticsPreview(null)}>
            {t("common.close")}
          </Button>
        }
      >
        {diagnosticsPreview ? (
          <div className="settings-diagnostics-preview">
            <dl className="settings-kv">
              <dt>{t("diagnostics.preview.appVersion")}</dt>
              <dd>{diagnosticsPreview.manifest.appVersion}</dd>
              <dt>{t("diagnostics.preview.engineVersion")}</dt>
              <dd>{diagnosticsPreview.manifest.engineVersion ?? t("common.none")}</dd>
              <dt>{t("diagnostics.preview.dialect")}</dt>
              <dd>{diagnosticsPreview.manifest.engineFlagDialect ?? t("common.none")}</dd>
              <dt>{t("diagnostics.preview.windowsVersion")}</dt>
              <dd>{diagnosticsPreview.manifest.windowsVersion}</dd>
              <dt>{t("diagnostics.preview.cpu")}</dt>
              <dd>
                {diagnosticsPreview.manifest.cpuModel
                  ? diagnosticsPreview.manifest.cpuCores
                    ? `${diagnosticsPreview.manifest.cpuModel} (${diagnosticsPreview.manifest.cpuCores} cores)`
                    : diagnosticsPreview.manifest.cpuModel
                  : t("common.none")}
              </dd>
              <dt>{t("diagnostics.preview.memory")}</dt>
              <dd>{formatMemoryBytes(diagnosticsPreview.manifest.memoryBytes)}</dd>
              <dt>{t("diagnostics.preview.gpus")}</dt>
              <dd>
                {diagnosticsPreview.manifest.gpus.length > 0
                  ? diagnosticsPreview.manifest.gpus.join(", ")
                  : t("common.none")}
              </dd>
              <dt>{t("diagnostics.preview.exportedAt")}</dt>
              <dd>{new Date(diagnosticsPreview.manifest.exportedAt).toLocaleString()}</dd>
            </dl>
            <p className="settings-card__body">
              {t("diagnostics.preview.eventCount", {
                count: diagnosticsPreview.events.length,
              })}
            </p>
            {diagnosticsPreview.events.length === 0 ? (
              <p className="settings-card__body">{t("diagnostics.preview.empty")}</p>
            ) : (
              <ul className="settings-diagnostics-events" role="list">
                {diagnosticsPreview.events.map((event) => (
                  <li key={event.seq} className="settings-diagnostics-events__row">
                    <code>
                      {event.component}.{event.action}
                    </code>
                    <span>
                      {event.severity}/{event.outcome}
                    </span>
                    {event.code ? <code>{event.code}</code> : null}
                  </li>
                ))}
              </ul>
            )}
          </div>
        ) : null}
      </Modal>
    </section>
  );
}
