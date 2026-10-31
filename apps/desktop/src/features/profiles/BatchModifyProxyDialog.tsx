import { useEffect, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { Proxy } from "@veil/shared";
import { Banner, Button, Modal, Select } from "@veil/ui";
import { batchSetProfileProxy, listProxies } from "../../api/client";
import { formatProxyOption } from "../proxy/format";
import { ProxyFormDialog } from "../proxy/ProxyFormDialog";

interface BatchModifyProxyDialogProps {
  open: boolean;
  profileIds: string[];
  profileName?: string | undefined;
  initialProxyId?: string | null | undefined;
  onClose: () => void;
  onApplied: (changedCount: number) => void;
}

/**
 * Pick a proxy (or "none") and assign it to N already-selected profiles in one shot.
 * Returns the number of rows the backend actually updated.
 */
export function BatchModifyProxyDialog({
  open,
  profileIds,
  profileName,
  initialProxyId,
  onClose,
  onApplied,
}: BatchModifyProxyDialogProps): ReactNode {
  const t = useT();
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [value, setValue] = useState<string>("");
  const [createOpen, setCreateOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setValue(initialProxyId ?? "");
    setCreateOpen(false);
    setErrorCode(null);
    setSubmitting(false);
    listProxies()
      .then(setProxies)
      .catch(() => setProxies([]));
  }, [initialProxyId, open]);

  async function apply(): Promise<void> {
    if (submitting) return;
    setSubmitting(true);
    setErrorCode(null);
    try {
      const changed = await batchSetProfileProxy(profileIds, value === "" ? null : value);
      onApplied(changed);
    } catch {
      setErrorCode("app.unknown");
      setSubmitting(false);
    }
  }

  function handleProxyCreated(proxy: Proxy): void {
    setProxies((prev) =>
      [...prev.filter((item) => item.id !== proxy.id), proxy].sort((a, b) =>
        a.name.localeCompare(b.name),
      ),
    );
    setValue(proxy.id);
    setCreateOpen(false);
  }

  const title = profileName
    ? t("profiles.modifyProxy.singleTitle", { name: profileName })
    : t("profiles.batch.modifyProxy.title", { count: profileIds.length });

  return (
    <>
      <Modal
        open={open}
        title={title}
        onClose={submitting ? () => undefined : onClose}
        size="sm"
        dismissOnBackdrop={!submitting}
        dismissOnEscape={!submitting}
        footer={
          <>
            <Button variant="secondary" onClick={onClose} disabled={submitting}>
              {t("common.cancel")}
            </Button>
            <Button variant="primary" onClick={() => void apply()} disabled={submitting}>
              {t("common.save")}
            </Button>
          </>
        }
      >
        <div className="veil-form">
          {errorCode ? (
            <Banner tone="error" title={t(`errors.${errorCode}`)}>
              {null}
            </Banner>
          ) : null}
          <div className="profiles-proxy-dialog__picker">
            <Select
              label={t("profiles.modifyProxy.select")}
              value={value}
              onChange={(e) => setValue(e.currentTarget.value)}
              options={[
                { value: "", label: t("profiles.noProxy") },
                ...proxies.map((p) => ({ value: p.id, label: formatProxyOption(p) })),
              ]}
            />
            <Button variant="secondary" onClick={() => setCreateOpen(true)} disabled={submitting}>
              {t("profiles.modifyProxy.new")}
            </Button>
          </div>
        </div>
      </Modal>
      <ProxyFormDialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onSaved={handleProxyCreated}
      />
    </>
  );
}
