import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { Proxy, ProxyTestResult } from "@veil/shared";
import { Button, ConfirmDialog, Modal, TextField } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import {
  deleteProxy,
  listProxies,
  revealProxyCredentials,
  testProxy,
  type ProxyCredentials,
  type SensitiveActionInput,
} from "../../api/client";
import { SensitiveActionDialog } from "../security/SensitiveActionDialog";
import { ProxyBatchImportDialog } from "./ProxyBatchImportDialog";
import { ProxyFormDialog } from "./ProxyFormDialog";
import { ProxyRow } from "./ProxyRow";

type DialogState =
  { kind: "closed" } | { kind: "create" } | { kind: "batch" } | { kind: "edit"; proxy: Proxy };

type ConfirmState = { kind: "closed" } | { kind: "delete"; proxy: Proxy };

interface TestEntry {
  state: "running" | "result";
  result: ProxyTestResult | null;
}

/**
 * Proxy section: list + CRUD + per-row connectivity test.
 *
 * The list is intentionally a flat stack, not virtualized — a real user typically owns
 * 10-50 proxies, not thousands. If that assumption breaks we can swap to a virtualizer
 * later without changing this file's shape.
 */
export function ProxyPage(): ReactNode {
  const t = useT();
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [loading, setLoading] = useState(true);
  const [dialog, setDialog] = useState<DialogState>({ kind: "closed" });
  const [confirm, setConfirm] = useState<ConfirmState>({ kind: "closed" });
  const [tests, setTests] = useState<Record<string, TestEntry>>({});
  const [revealTarget, setRevealTarget] = useState<Proxy | null>(null);
  const [revealed, setRevealed] = useState<{
    proxy: Proxy;
    credentials: ProxyCredentials;
  } | null>(null);
  const [importedCount, setImportedCount] = useState<number | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setProxies(await listProxies());
    } catch {
      setProxies([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  async function handleTest(proxy: Proxy): Promise<void> {
    setTests((prev) => ({ ...prev, [proxy.id]: { state: "running", result: null } }));
    try {
      const result = await testProxy(proxy.id);
      setTests((prev) => ({ ...prev, [proxy.id]: { state: "result", result } }));
      if (result.ok) await reload();
    } catch {
      setTests((prev) => ({
        ...prev,
        [proxy.id]: {
          state: "result",
          result: {
            ok: false,
            ip: null,
            country: null,
            countryCode: null,
            region: null,
            city: null,
            timezone: null,
            latencyMs: null,
            code: "proxy.testFailed",
          },
        },
      }));
    }
  }

  async function handleSaved(): Promise<void> {
    setDialog({ kind: "closed" });
    await reload();
  }

  async function handleBatchImported(count: number): Promise<void> {
    setImportedCount(count);
    await reload();
  }

  async function confirmDelete(): Promise<void> {
    if (confirm.kind !== "delete") return;
    const { proxy } = confirm;
    setConfirm({ kind: "closed" });
    try {
      await deleteProxy(proxy.id);
    } catch {
      /* surfaced by reload; the row simply stays. */
    }
    setTests((prev) => {
      const { [proxy.id]: _, ...rest } = prev;
      return rest;
    });
    await reload();
  }

  async function handleRevealCredentials(input: SensitiveActionInput): Promise<void> {
    if (!revealTarget) return;
    const credentials = await revealProxyCredentials(revealTarget.id, input);
    setRevealed({ proxy: revealTarget, credentials });
  }

  const hasProxies = proxies.length > 0;

  return (
    <section className="veil-section">
      <header className="veil-section__header">
        <h2 className="veil-section__title">{t("proxy.title")}</h2>
        <span className="veil-section__spacer" />
        <Button variant="secondary" size="sm" onClick={() => void reload()}>
          <Icon name="refresh" size={14} />
          {t("toolbar.refresh")}
        </Button>
        <Button variant="secondary" size="sm" onClick={() => setDialog({ kind: "batch" })}>
          <Icon name="import" size={14} />
          {t("proxy.batch.add")}
        </Button>
        <Button variant="primary" size="sm" onClick={() => setDialog({ kind: "create" })}>
          <Icon name="plus" size={14} />
          {t("proxy.create")}
        </Button>
      </header>

      <p className="proxy-subtitle">{t("proxy.subtitle")}</p>
      {importedCount !== null ? (
        <div className="proxy-import-result" role="status">
          {t("proxy.batch.imported", { count: importedCount })}
        </div>
      ) : null}

      {!hasProxies && !loading ? (
        <div className="veil-empty">
          <span className="veil-empty__title">{t("proxy.empty.title")}</span>
          <span>{t("proxy.empty.body")}</span>
          <Button variant="primary" onClick={() => setDialog({ kind: "create" })}>
            <Icon name="plus" size={14} />
            {t("proxy.create")}
          </Button>
        </div>
      ) : (
        <div className="proxy-list">
          {proxies.map((proxy) => {
            const tracker = tests[proxy.id];
            return (
              <ProxyRow
                key={proxy.id}
                proxy={proxy}
                testState={tracker?.state ?? "idle"}
                testResult={tracker?.result ?? null}
                onTest={(p) => void handleTest(p)}
                onEdit={(p) => setDialog({ kind: "edit", proxy: p })}
                onReveal={(p) => setRevealTarget(p)}
                onDelete={(p) => setConfirm({ kind: "delete", proxy: p })}
              />
            );
          })}
        </div>
      )}

      <ProxyFormDialog
        open={dialog.kind === "create" || dialog.kind === "edit"}
        proxy={dialog.kind === "edit" ? dialog.proxy : undefined}
        onClose={() => setDialog({ kind: "closed" })}
        onSaved={() => void handleSaved()}
      />

      <ProxyBatchImportDialog
        open={dialog.kind === "batch"}
        onClose={() => setDialog({ kind: "closed" })}
        onImported={(count) => void handleBatchImported(count)}
      />

      <ConfirmDialog
        open={confirm.kind !== "closed"}
        danger
        title={t("proxy.confirm.delete.title")}
        body={t("proxy.confirm.delete.body")}
        confirmLabel={t("common.delete")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void confirmDelete()}
        onClose={() => setConfirm({ kind: "closed" })}
      />

      <SensitiveActionDialog
        open={revealTarget !== null}
        title={t("proxy.credentials.title")}
        body={t("proxy.credentials.body")}
        confirmLabel={t("proxy.credentials.reveal")}
        onClose={() => setRevealTarget(null)}
        onConfirm={handleRevealCredentials}
      />

      <Modal
        open={revealed !== null}
        title={t("proxy.credentials.title")}
        onClose={() => setRevealed(null)}
        size="sm"
        footer={
          <Button variant="primary" onClick={() => setRevealed(null)}>
            {t("common.close")}
          </Button>
        }
      >
        <div className="proxy-credentials">
          <p className="proxy-subtitle">{revealed?.proxy.name}</p>
          <TextField
            label={t("proxy.field.username")}
            value={revealed?.credentials.username ?? ""}
            readOnly
          />
          <TextField
            label={t("proxy.field.password")}
            value={revealed?.credentials.password ?? ""}
            readOnly
          />
        </div>
      </Modal>
    </section>
  );
}
