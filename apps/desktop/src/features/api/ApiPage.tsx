import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Badge, Banner, Button, Select, Spinner, TextField } from "@veil/ui";
import {
  AppError,
  generateLocalApiKey,
  getLocalApiSettings,
  listLocalApiKeys,
  rotateLocalApiKey,
  updateLocalApiSettings,
  type LocalApiBindMode,
  type LocalApiKey,
  type LocalApiSettings,
  type SensitiveActionInput,
  type UpdateLocalApiSettingsInput,
} from "../../api/client";
import { Icon } from "../../shell/Icon";
import { SensitiveActionDialog } from "../security/SensitiveActionDialog";
import {
  apiEndpoints,
  apiGroups,
  buildSampleRequest,
  endpointsByGroup,
  getEndpointById,
  type ApiEndpoint,
  type ApiGroupId,
  type ApiMethod,
} from "./catalog";

type ApiTab = "service" | "endpoints";
type KeyVerificationAction = "generate" | "rotate";

interface RunnerResult {
  endpoint: ApiEndpoint;
  keySuffix: string;
  sensitiveVerified: boolean;
}

const DEFAULT_SETTINGS: LocalApiSettings = {
  enabled: false,
  port: 17688,
  bindMode: "loopback",
  updatedAt: null,
};

export function ApiPage(): ReactNode {
  const t = useT();
  const [tab, setTab] = useState<ApiTab>("service");
  const [settings, setSettings] = useState<LocalApiSettings>(DEFAULT_SETTINGS);
  const [draftSettings, setDraftSettings] = useState<UpdateLocalApiSettingsInput>(DEFAULT_SETTINGS);
  const [keys, setKeys] = useState<LocalApiKey[]>([]);
  const [selectedKeyId, setSelectedKeyId] = useState("");
  const [lastFullKey, setLastFullKey] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [messageKey, setMessageKey] = useState<string | null>(null);
  const [errorKey, setErrorKey] = useState<string | null>(null);
  const [pendingSettings, setPendingSettings] = useState<UpdateLocalApiSettingsInput | null>(null);
  const [keyVerificationAction, setKeyVerificationAction] = useState<KeyVerificationAction | null>(
    null,
  );
  const [selectedEndpointId, setSelectedEndpointId] = useState(apiEndpoints[0]!.id);
  const [endpointSearch, setEndpointSearch] = useState("");
  const [runnerResult, setRunnerResult] = useState<RunnerResult | null>(null);
  const [runnerVerificationOpen, setRunnerVerificationOpen] = useState(false);

  useEffect(() => {
    void refresh();
  }, []);

  const selectedEndpoint = getEndpointById(selectedEndpointId);
  const selectedKey = keys.find((key) => key.id === selectedKeyId) ?? keys[0] ?? null;
  const baseUrl = `http://${settings.bindMode === "lan" ? "0.0.0.0" : "127.0.0.1"}:${settings.port}`;
  const sampleRequest = buildSampleRequest(selectedEndpoint, baseUrl, selectedKey?.suffix ?? "");

  async function refresh(): Promise<void> {
    try {
      const [nextSettings, nextKeys] = await Promise.all([
        getLocalApiSettings(),
        listLocalApiKeys(),
      ]);
      setSettings(nextSettings);
      setDraftSettings({
        enabled: nextSettings.enabled,
        port: nextSettings.port,
        bindMode: nextSettings.bindMode,
      });
      setKeys(nextKeys);
      setSelectedKeyId((current) => current || nextKeys[0]?.id || "");
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setErrorKey(`errors.${code}`);
    }
  }

  async function saveSettings(
    input: UpdateLocalApiSettingsInput,
    verification: SensitiveActionInput | null = null,
  ): Promise<void> {
    setBusy(true);
    setErrorKey(null);
    setMessageKey(null);
    try {
      const saved = await updateLocalApiSettings(input, verification);
      setSettings(saved);
      setDraftSettings({ enabled: saved.enabled, port: saved.port, bindMode: saved.bindMode });
      setMessageKey("api.message.settingsSaved");
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setErrorKey(`errors.${code}`);
      throw err;
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveSettings(): Promise<void> {
    const normalized = normalizeDraftSettings(draftSettings);
    if (settings.bindMode !== "lan" && normalized.bindMode === "lan") {
      setPendingSettings(normalized);
      return;
    }
    await saveSettings(normalized);
  }

  async function handleGenerateKey(): Promise<void> {
    setKeyVerificationAction("generate");
  }

  async function handleRotateKey(): Promise<void> {
    if (!selectedKey) return;
    setKeyVerificationAction("rotate");
  }

  async function issueApiKey(verification: SensitiveActionInput): Promise<void> {
    setBusy(true);
    setErrorKey(null);
    setMessageKey(null);
    try {
      const issue =
        keyVerificationAction === "rotate" && selectedKey
          ? await rotateLocalApiKey(selectedKey.id, verification)
          : await generateLocalApiKey("Default", verification);
      const nextKeys = await listLocalApiKeys();
      setKeys(nextKeys);
      setSelectedKeyId(issue.key.id);
      setLastFullKey(issue.fullKey);
      setMessageKey(
        keyVerificationAction === "rotate" ? "api.message.keyRotated" : "api.message.keyGenerated",
      );
      setKeyVerificationAction(null);
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setErrorKey(`errors.${code}`);
    } finally {
      setBusy(false);
    }
  }

  async function copyFullKey(): Promise<void> {
    if (!lastFullKey) return;
    await navigator.clipboard.writeText(lastFullKey);
    setMessageKey("api.message.keyCopied");
  }

  async function copyBaseUrl(): Promise<void> {
    await navigator.clipboard.writeText(baseUrl);
    setMessageKey("api.message.baseUrlCopied");
  }

  function handleRunEndpoint(): void {
    setRunnerResult(null);
    if (selectedEndpoint.protection === "sensitive") {
      setRunnerVerificationOpen(true);
      return;
    }
    setRunnerResult({
      endpoint: selectedEndpoint,
      keySuffix: selectedKey?.suffix ?? "",
      sensitiveVerified: false,
    });
  }

  async function verifyAndRunEndpoint(_input: SensitiveActionInput): Promise<void> {
    setRunnerResult({
      endpoint: selectedEndpoint,
      keySuffix: selectedKey?.suffix ?? "",
      sensitiveVerified: true,
    });
  }

  return (
    <section className="veil-section api-page">
      <header className="veil-section__header api-page__header">
        <div>
          <h2 className="veil-section__title">{t("api.title")}</h2>
          <div className="api-statusline">
            <Badge
              tone={
                settings.enabled ? (settings.bindMode === "lan" ? "warning" : "success") : "neutral"
              }
              dot
            >
              {t(statusKey(settings))}
            </Badge>
            <code>{settings.enabled ? baseUrl : t("api.status.disabledAddress")}</code>
          </div>
        </div>
        <span className="veil-section__spacer" />
        <Button size="sm" variant="secondary" onClick={() => void copyBaseUrl()}>
          <Icon name="copy" size={14} />
          {t("api.docs.copyBaseUrl")}
        </Button>
      </header>

      <div className="api-tabs" role="tablist" aria-label={t("api.tabs.label")}>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "service"}
          className="api-tab"
          onClick={() => setTab("service")}
        >
          {t("api.tab.service")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "endpoints"}
          className="api-tab"
          onClick={() => setTab("endpoints")}
        >
          {t("api.tab.endpoints")}
        </button>
      </div>

      {messageKey ? <Banner tone="success">{t(messageKey)}</Banner> : null}
      {errorKey ? <Banner tone="error">{t(errorKey)}</Banner> : null}

      {tab === "service" ? (
        <ServiceConfiguration
          settings={settings}
          draftSettings={draftSettings}
          setDraftSettings={setDraftSettings}
          keys={keys}
          selectedKeyId={selectedKeyId}
          setSelectedKeyId={setSelectedKeyId}
          lastFullKey={lastFullKey}
          busy={busy}
          onSaveSettings={() => void handleSaveSettings()}
          onGenerateKey={() => void handleGenerateKey()}
          onRotateKey={() => void handleRotateKey()}
          onCopyFullKey={() => void copyFullKey()}
        />
      ) : (
        <EndpointPreview
          settings={settings}
          keys={keys}
          selectedKeyId={selectedKeyId}
          setSelectedKeyId={setSelectedKeyId}
          endpointSearch={endpointSearch}
          setEndpointSearch={setEndpointSearch}
          selectedEndpoint={selectedEndpoint}
          selectedEndpointId={selectedEndpointId}
          setSelectedEndpointId={setSelectedEndpointId}
          sampleRequest={sampleRequest}
          runnerResult={runnerResult}
          onRunEndpoint={handleRunEndpoint}
        />
      )}

      <SensitiveActionDialog
        open={pendingSettings !== null}
        title={t("api.security.external.title")}
        body={t("api.security.external.body")}
        confirmLabel={t("common.confirm")}
        onClose={() => setPendingSettings(null)}
        onConfirm={async (verification) => {
          if (!pendingSettings) return;
          await saveSettings(pendingSettings, verification);
          setPendingSettings(null);
        }}
      />

      <SensitiveActionDialog
        open={runnerVerificationOpen}
        title={t("api.runner.sensitive.title")}
        body={t("api.runner.sensitive.body")}
        confirmLabel={t("api.runner.send")}
        onClose={() => setRunnerVerificationOpen(false)}
        onConfirm={verifyAndRunEndpoint}
      />

      <SensitiveActionDialog
        open={keyVerificationAction !== null}
        title={t("api.keys.verify.title")}
        body={t("api.keys.verify.body")}
        confirmLabel={
          keyVerificationAction === "rotate" ? t("api.keys.rotate") : t("api.keys.generate")
        }
        onClose={() => setKeyVerificationAction(null)}
        onConfirm={issueApiKey}
      />
    </section>
  );
}

interface ServiceConfigurationProps {
  settings: LocalApiSettings;
  draftSettings: UpdateLocalApiSettingsInput;
  setDraftSettings: (settings: UpdateLocalApiSettingsInput) => void;
  keys: LocalApiKey[];
  selectedKeyId: string;
  setSelectedKeyId: (id: string) => void;
  lastFullKey: string | null;
  busy: boolean;
  onSaveSettings: () => void;
  onGenerateKey: () => void;
  onRotateKey: () => void;
  onCopyFullKey: () => void;
}

function ServiceConfiguration({
  settings,
  draftSettings,
  setDraftSettings,
  keys,
  selectedKeyId,
  setSelectedKeyId,
  lastFullKey,
  busy,
  onSaveSettings,
  onGenerateKey,
  onRotateKey,
  onCopyFullKey,
}: ServiceConfigurationProps): ReactNode {
  const t = useT();
  const selectedKey = keys.find((key) => key.id === selectedKeyId) ?? keys[0] ?? null;
  return (
    <div className="api-config-grid">
      <section className="api-panel">
        <div className="api-panel__head">
          <h3>{t("api.service.title")}</h3>
          <Badge tone={settings.enabled ? "success" : "neutral"} dot>
            {t(settings.enabled ? "api.service.enabled" : "api.service.disabled")}
          </Badge>
        </div>
        <label className="api-check">
          <input
            type="checkbox"
            checked={draftSettings.enabled}
            onChange={(event) =>
              setDraftSettings({ ...draftSettings, enabled: event.currentTarget.checked })
            }
          />
          <span>{t("api.service.enable")}</span>
        </label>
        <div className="api-form-row">
          <TextField
            type="number"
            label={t("api.service.port")}
            value={draftSettings.port}
            min={1024}
            max={65535}
            onChange={(event) =>
              setDraftSettings({
                ...draftSettings,
                port: Number.parseInt(event.currentTarget.value, 10) || 17688,
              })
            }
          />
          <Select
            label={t("api.service.bindMode")}
            value={draftSettings.bindMode}
            onChange={(event) =>
              setDraftSettings({
                ...draftSettings,
                bindMode: event.currentTarget.value as LocalApiBindMode,
              })
            }
            options={[
              { value: "loopback", label: t("api.service.bind.loopback") },
              { value: "lan", label: t("api.service.bind.lan") },
            ]}
          />
        </div>
        <p className="api-panel__note">{t("api.service.externalNote")}</p>
        <div className="api-actions">
          <Button
            variant="primary"
            size="sm"
            disabled={busy}
            onClick={onSaveSettings}
            aria-label={busy ? t("common.loading") : undefined}
          >
            {busy ? <Spinner size={14} label={t("common.loading")} /> : t("common.save")}
          </Button>
        </div>
      </section>

      <section className="api-panel">
        <div className="api-panel__head">
          <h3>{t("api.keys.title")}</h3>
          <Badge tone={keys.length > 0 ? "success" : "neutral"}>
            {keys.length > 0 ? t("api.keys.generated") : t("api.keys.none")}
          </Badge>
        </div>
        <Select
          label={t("api.keys.select")}
          value={selectedKey?.id ?? ""}
          placeholder={t("api.keys.emptySelect")}
          onChange={(event) => setSelectedKeyId(event.currentTarget.value)}
          options={keys.map((key) => ({
            value: key.id,
            label: `${key.name} ...${key.suffix}`,
          }))}
        />
        {selectedKey ? (
          <dl className="api-kv">
            <dt>{t("api.keys.suffix")}</dt>
            <dd>...{selectedKey.suffix}</dd>
            <dt>{t("api.keys.createdAt")}</dt>
            <dd>{formatTime(selectedKey.createdAt)}</dd>
            <dt>{t("api.keys.lastUsedAt")}</dt>
            <dd>
              {selectedKey.lastUsedAt ? formatTime(selectedKey.lastUsedAt) : t("api.keys.never")}
            </dd>
          </dl>
        ) : null}
        {lastFullKey ? (
          <div className="api-key-once">
            <span>{t("api.keys.fullKeyOnce")}</span>
            <code>{lastFullKey}</code>
          </div>
        ) : (
          <p className="api-panel__note">{t("api.keys.onceNote")}</p>
        )}
        <div className="api-actions">
          <Button variant="primary" size="sm" disabled={busy} onClick={onGenerateKey}>
            {t("api.keys.generate")}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            disabled={busy || !selectedKey}
            onClick={onRotateKey}
          >
            {t("api.keys.rotate")}
          </Button>
          <Button variant="secondary" size="sm" disabled={!lastFullKey} onClick={onCopyFullKey}>
            <Icon name="copy" size={14} />
            {t("api.keys.copy")}
          </Button>
        </div>
      </section>

      <section className="api-panel api-panel--wide">
        <div className="api-panel__head">
          <h3>{t("api.security.title")}</h3>
          <Badge tone="warning">{t("api.protection.sensitive")}</Badge>
        </div>
        <div className="api-sensitive-list">
          {[
            "api.security.cookies",
            "api.security.credentials",
            "api.security.profileData",
            "api.security.cache",
            "api.security.fullParams",
            "api.security.externalAccess",
          ].map((key) => (
            <span key={key}>{t(key)}</span>
          ))}
        </div>
        <p className="api-panel__note">{t("api.security.note")}</p>
      </section>
    </div>
  );
}

interface EndpointPreviewProps {
  settings: LocalApiSettings;
  keys: LocalApiKey[];
  selectedKeyId: string;
  setSelectedKeyId: (id: string) => void;
  endpointSearch: string;
  setEndpointSearch: (value: string) => void;
  selectedEndpoint: ApiEndpoint;
  selectedEndpointId: string;
  setSelectedEndpointId: (id: string) => void;
  sampleRequest: string;
  runnerResult: RunnerResult | null;
  onRunEndpoint: () => void;
}

function EndpointPreview({
  settings,
  keys,
  selectedKeyId,
  setSelectedKeyId,
  endpointSearch,
  setEndpointSearch,
  selectedEndpoint,
  selectedEndpointId,
  setSelectedEndpointId,
  sampleRequest,
  runnerResult,
  onRunEndpoint,
}: EndpointPreviewProps): ReactNode {
  const t = useT();
  const filteredGroups = useMemo(
    () =>
      apiGroups
        .map((group) => ({
          group,
          endpoints: endpointsByGroup(group.id).filter((endpoint) =>
            endpointMatches(endpoint, endpointSearch, t(endpoint.titleKey), group.id),
          ),
        }))
        .filter((entry) => entry.endpoints.length > 0),
    [endpointSearch, t],
  );
  const selectedKey = keys.find((key) => key.id === selectedKeyId) ?? keys[0] ?? null;

  return (
    <div className="api-workbench">
      <aside className="api-catalog">
        <TextField
          label={t("api.catalog.search")}
          placeholder={t("api.catalog.searchPlaceholder")}
          value={endpointSearch}
          onChange={(event) => setEndpointSearch(event.currentTarget.value)}
        />
        <div className="api-catalog__list">
          {filteredGroups.map(({ group, endpoints }) => (
            <div key={group.id} className="api-catalog__group">
              <div className="api-catalog__group-title">{t(group.titleKey)}</div>
              {endpoints.map((endpoint) => (
                <button
                  key={endpoint.id}
                  type="button"
                  className="api-catalog__item"
                  data-active={selectedEndpointId === endpoint.id}
                  onClick={() => setSelectedEndpointId(endpoint.id)}
                >
                  <MethodBadge method={endpoint.method} />
                  <span>{t(endpoint.titleKey)}</span>
                  {endpoint.protection === "sensitive" ? (
                    <span className="api-catalog__lock">{t("api.protection.lock")}</span>
                  ) : null}
                </button>
              ))}
            </div>
          ))}
        </div>
      </aside>

      <main className="api-detail">
        <div className="api-endpoint-title">
          <div>
            <h3>{t(selectedEndpoint.titleKey)}</h3>
            <p>{t(selectedEndpoint.descriptionKey)}</p>
          </div>
          <Badge tone={selectedEndpoint.protection === "sensitive" ? "warning" : "success"}>
            {t(
              selectedEndpoint.protection === "sensitive"
                ? "api.protection.sensitive"
                : "api.protection.standard",
            )}
          </Badge>
        </div>
        <div className="api-path">
          <MethodBadge method={selectedEndpoint.method} />
          <code>{selectedEndpoint.path}</code>
        </div>

        <section className="api-doc-section">
          <h4>{t("api.detail.params")}</h4>
          <div className="api-param-card">
            <div className="api-param-card__title">{t("api.detail.headers")}</div>
            <div className="api-param-row">
              <code>Authorization</code>
              <span>Bearer &lt;api_key&gt;</span>
              <Badge tone="warning">{t("api.detail.required")}</Badge>
            </div>
            {selectedEndpoint.protection === "sensitive" ? (
              <div className="api-param-row">
                <code>X-Veil-Sensitive-Token</code>
                <span>{t("api.detail.sensitiveToken")}</span>
                <Badge tone="warning">{t("api.detail.required")}</Badge>
              </div>
            ) : null}
          </div>
          {selectedEndpoint.queryParams?.length ? (
            <div className="api-param-card">
              <div className="api-param-card__title">{t("api.detail.query")}</div>
              {selectedEndpoint.queryParams.map((param) => (
                <div key={param.name} className="api-param-row">
                  <code>{param.name}</code>
                  <span>{param.type}</span>
                  <span>{param.example}</span>
                </div>
              ))}
            </div>
          ) : null}
        </section>

        <section className="api-doc-section">
          <h4>{t("api.detail.samples")}</h4>
          <div className="api-sample-tabs" aria-label={t("api.detail.samples")}>
            <span data-active="true">JavaScript</span>
            <span>Shell</span>
            <span>Python</span>
            <span>HTTP</span>
          </div>
          <pre className="api-code">
            <code>{sampleRequest}</code>
          </pre>
        </section>

        <section className="api-doc-section">
          <h4>{t("api.detail.response")}</h4>
          <pre className="api-code">
            <code>{selectedEndpoint.responseExample}</code>
          </pre>
        </section>
      </main>

      <aside className="api-runner">
        <div className="api-runner__head">
          <h3>{t("api.runner.title")}</h3>
          <Badge tone={settings.enabled ? "success" : "neutral"} dot>
            {t(settings.enabled ? "api.service.enabled" : "api.service.disabled")}
          </Badge>
        </div>
        <Select
          label={t("api.runner.apiKey")}
          value={selectedKey?.id ?? ""}
          placeholder={t("api.keys.emptySelect")}
          onChange={(event) => setSelectedKeyId(event.currentTarget.value)}
          options={keys.map((key) => ({
            value: key.id,
            label: `${key.name} ...${key.suffix}`,
          }))}
        />
        <div className="api-runner__request">
          <MethodBadge method={selectedEndpoint.method} />
          <code>{selectedEndpoint.path}</code>
        </div>
        <Button
          variant="primary"
          size="sm"
          disabled={!settings.enabled || !selectedKey}
          onClick={onRunEndpoint}
        >
          {t("api.runner.send")}
        </Button>
        {!settings.enabled ? <Banner tone="warning">{t("api.runner.disabled")}</Banner> : null}
        {!selectedKey ? <Banner tone="warning">{t("api.runner.noKey")}</Banner> : null}
        <div className="api-runner__section">
          <h4>{t("api.runner.headers")}</h4>
          <div className="api-runner__kv">
            <span>Authorization</span>
            <code>
              Bearer {selectedKey ? `...${selectedKey.suffix}` : t("api.runner.noKeyValue")}
            </code>
          </div>
        </div>
        <div className="api-runner__section">
          <h4>{t("api.runner.body")}</h4>
          <pre className="api-runner__mini">
            <code>{selectedEndpoint.bodyExample ?? "{}"}</code>
          </pre>
        </div>
        <div className="api-runner__result">
          <h4>{t("api.runner.result")}</h4>
          {runnerResult ? (
            <pre className="api-runner__mini">
              <code>{runnerOutput(runnerResult, t("api.runner.pendingMessage"))}</code>
            </pre>
          ) : (
            <div className="api-runner__empty">
              <Icon name="play" size={20} />
              <span>{t("api.runner.empty")}</span>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}

function MethodBadge({ method }: { method: ApiMethod }): ReactNode {
  return (
    <span className="api-method" data-method={method}>
      {method}
    </span>
  );
}

function normalizeDraftSettings(input: UpdateLocalApiSettingsInput): UpdateLocalApiSettingsInput {
  return {
    enabled: input.enabled,
    port: clampPort(input.port),
    bindMode: input.bindMode === "lan" ? "lan" : "loopback",
  };
}

function clampPort(port: number): number {
  if (!Number.isFinite(port)) return 17688;
  return Math.max(1, Math.min(65535, Math.round(port)));
}

function statusKey(settings: LocalApiSettings): string {
  if (!settings.enabled) return "api.status.disabled";
  return settings.bindMode === "lan" ? "api.status.lan" : "api.status.loopback";
}

function formatTime(value: string): string {
  return new Date(value).toLocaleString();
}

function endpointMatches(
  endpoint: ApiEndpoint,
  search: string,
  localizedTitle: string,
  groupId: ApiGroupId,
): boolean {
  const needle = search.trim().toLowerCase();
  if (!needle) return true;
  return [localizedTitle, endpoint.method, endpoint.path, endpoint.id, groupId]
    .join(" ")
    .toLowerCase()
    .includes(needle);
}

function runnerOutput(result: RunnerResult, message: string): string {
  return JSON.stringify(
    {
      success: false,
      request: {
        endpointId: result.endpoint.id,
        method: result.endpoint.method,
        path: result.endpoint.path,
        apiKey: result.keySuffix ? `...${result.keySuffix}` : null,
        sensitiveVerified: result.sensitiveVerified,
      },
      error: {
        code: "localApi.endpointPending",
        message,
      },
      exampleResponse: JSON.parse(result.endpoint.responseExample),
    },
    null,
    2,
  );
}
