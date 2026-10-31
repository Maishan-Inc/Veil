import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { CreateProxyInput, Proxy, ProxyType } from "@veil/shared";
import { Banner, Button, Modal, Select, TextField } from "@veil/ui";
import { AppError, createProxy, updateProxy, type UpdateProxyInput } from "../../api/client";

export interface ProxyFormDialogProps {
  open: boolean;
  /** Editing mode when provided; create mode when undefined. */
  proxy?: Proxy | undefined;
  onClose: () => void;
  onSaved: (proxy: Proxy) => void;
}

interface FormState {
  name: string;
  type: ProxyType;
  host: string;
  port: string;
  username: string;
  password: string;
  country: string;
}

function emptyForm(): FormState {
  return {
    name: "",
    type: "http",
    host: "",
    port: "",
    username: "",
    password: "",
    country: "",
  };
}

function fromProxy(p: Proxy): FormState {
  return {
    name: p.name,
    type: p.type,
    host: p.host,
    port: String(p.port),
    username: p.username ?? "",
    // Never echo a password back to the form — the backend doesn't expose it.
    password: "",
    country: p.country ?? "",
  };
}

/**
 * Create / edit a proxy. The password input behaves differently per mode:
 *   - create: empty means "no password"
 *   - edit:   empty means "leave the existing password unchanged"
 *
 * The placeholder + helper text make this contract visible to the user.
 */
export function ProxyFormDialog({
  open,
  proxy,
  onClose,
  onSaved,
}: ProxyFormDialogProps): ReactNode {
  const t = useT();
  const editing = Boolean(proxy);
  const [form, setForm] = useState<FormState>(() => (proxy ? fromProxy(proxy) : emptyForm()));
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm(proxy ? fromProxy(proxy) : emptyForm());
    setErrorCode(null);
    setSubmitting(false);
  }, [open, proxy]);

  function patch<K extends keyof FormState>(key: K, value: FormState[K]): void {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (submitting) return;
    setSubmitting(true);
    setErrorCode(null);

    const port = Number.parseInt(form.port, 10);
    if (!Number.isFinite(port) || port <= 0 || port > 65535) {
      setErrorCode("proxy.invalid");
      setSubmitting(false);
      return;
    }

    try {
      const username = form.username.trim() || null;
      const country = form.country.trim() || null;
      if (editing && proxy) {
        // Empty password means "leave alone" in edit mode.
        const update: UpdateProxyInput = {
          name: form.name.trim(),
          type: form.type,
          host: form.host.trim(),
          port,
          username,
          password: form.password === "" ? null : form.password,
          country,
        };
        const saved = await updateProxy(proxy.id, update);
        onSaved(saved);
      } else {
        const create: CreateProxyInput = {
          name: form.name.trim(),
          type: form.type,
          host: form.host.trim(),
          port,
          username,
          password: form.password === "" ? null : form.password,
          country,
        };
        const saved = await createProxy(create);
        onSaved(saved);
      }
    } catch (err) {
      setErrorCode(err instanceof AppError ? err.code : "app.unknown");
      setSubmitting(false);
    }
  }

  return (
    <Modal
      open={open}
      title={t(editing ? "proxy.dialog.edit.title" : "proxy.dialog.create.title")}
      onClose={submitting ? () => undefined : onClose}
      size="md"
      dismissOnBackdrop={!submitting}
      dismissOnEscape={!submitting}
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={submitting}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" type="submit" form="veil-proxy-form" disabled={submitting}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <form id="veil-proxy-form" className="veil-form" onSubmit={(e) => void handleSubmit(e)}>
        {errorCode ? (
          <Banner tone="error" title={t(`errors.${errorCode}`)}>
            {null}
          </Banner>
        ) : null}

        <TextField
          label={t("proxy.field.name")}
          value={form.name}
          onChange={(e) => patch("name", e.currentTarget.value)}
          placeholder={t("proxy.field.name.placeholder")}
          autoFocus
          required
        />

        <Select
          label={t("proxy.field.kind")}
          value={form.type}
          onChange={(e) => patch("type", e.currentTarget.value as ProxyType)}
          options={[
            { value: "http", label: t("proxy.kind.http") },
            { value: "https", label: t("proxy.kind.https") },
            { value: "socks5", label: t("proxy.kind.socks5") },
          ]}
        />

        <div className="proxy-form__host-port">
          <TextField
            label={t("proxy.field.host")}
            value={form.host}
            onChange={(e) => patch("host", e.currentTarget.value)}
            placeholder={t("proxy.field.host.placeholder")}
            required
          />
          <TextField
            label={t("proxy.field.port")}
            value={form.port}
            onChange={(e) => patch("port", e.currentTarget.value)}
            placeholder="8080"
            inputMode="numeric"
            required
          />
        </div>

        <TextField
          label={t("proxy.field.username")}
          value={form.username}
          onChange={(e) => patch("username", e.currentTarget.value)}
          autoComplete="off"
        />

        <TextField
          type="password"
          label={t("proxy.field.password")}
          value={form.password}
          onChange={(e) => patch("password", e.currentTarget.value)}
          placeholder={
            editing && proxy?.hasPassword
              ? t("proxy.field.password.editPlaceholder")
              : t("proxy.field.password.placeholder")
          }
          autoComplete="new-password"
        />

        <TextField
          label={t("proxy.field.country")}
          value={form.country}
          onChange={(e) => patch("country", e.currentTarget.value)}
          placeholder={t("proxy.field.country.placeholder")}
        />

        {form.username || form.password ? (
          <Banner tone="warning" title={t("proxy.warning.credentialsLaunchUnsupported.title")}>
            {t("proxy.warning.credentialsLaunchUnsupported.body")}
          </Banner>
        ) : null}
      </form>
    </Modal>
  );
}
