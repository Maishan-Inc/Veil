import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Banner, Button, Modal, Spinner, TextField } from "@veil/ui";
import {
  AppError,
  getOnboardingStatus,
  type OnboardingStatus,
  type SensitiveActionInput,
} from "../../api/client";

interface SensitiveActionDialogProps {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  onClose: () => void;
  onConfirm: (input: SensitiveActionInput) => Promise<void>;
}

const FALLBACK_STATUS: OnboardingStatus = {
  completed: true,
  agreementAcceptedAt: null,
  encryptionMode: "none",
  passwordRequired: false,
  twoFactorEnabled: false,
  twoFactorPending: false,
  mnemonicEnabled: false,
  developer: "Maishan Inc.",
};

export function SensitiveActionDialog({
  open,
  title,
  body,
  confirmLabel,
  onClose,
  onConfirm,
}: SensitiveActionDialogProps): ReactNode {
  const t = useT();
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setPassword("");
    setTwoFactorCode("");
    setErrorKey(null);
    setStatus(null);
    getOnboardingStatus()
      .then(setStatus)
      .catch(() => setStatus(FALLBACK_STATUS));
  }, [open]);

  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    setBusy(true);
    setErrorKey(null);
    try {
      await onConfirm({
        password: status?.passwordRequired ? password : null,
        twoFactorCode: status?.twoFactorEnabled ? twoFactorCode : null,
      });
      onClose();
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setErrorKey(`errors.${code}`);
    } finally {
      setBusy(false);
    }
  }

  const needsPassword = status?.passwordRequired ?? false;
  const needsTwoFactor = status?.twoFactorEnabled ?? false;

  return (
    <Modal
      open={open}
      title={title}
      onClose={onClose}
      size="sm"
      footer={
        <>
          <Button variant="subtle" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button
            type="submit"
            form="veil-sensitive-action-form"
            variant="primary"
            disabled={
              busy ||
              (needsPassword && password.trim().length === 0) ||
              (needsTwoFactor && twoFactorCode.trim().length === 0)
            }
            aria-label={busy ? t("common.loading") : undefined}
          >
            {busy ? <Spinner size={16} label={t("common.loading")} /> : confirmLabel}
          </Button>
        </>
      }
    >
      <form
        id="veil-sensitive-action-form"
        className="veil-form"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <p className="sensitive-action__body">{body}</p>
        {status === null ? (
          <p>
            <Spinner size={16} label={t("common.loading")} />
          </p>
        ) : null}
        {status && !needsPassword ? (
          <Banner tone="info">{t("security.verify.noPasswordRequired")}</Banner>
        ) : null}
        {needsPassword ? (
          <TextField
            type="password"
            label={t("security.verify.password")}
            placeholder={t("security.verify.password.placeholder")}
            value={password}
            onChange={(event) => setPassword(event.currentTarget.value)}
            autoComplete="current-password"
          />
        ) : null}
        {needsTwoFactor ? (
          <TextField
            inputMode="numeric"
            label={t("security.verify.2fa")}
            placeholder={t("security.verify.2fa.placeholder")}
            value={twoFactorCode}
            onChange={(event) => setTwoFactorCode(event.currentTarget.value)}
            autoComplete="one-time-code"
          />
        ) : null}
        {status?.twoFactorPending ? (
          <Banner tone="warning">{t("security.verify.2faPending")}</Banner>
        ) : null}
        {errorKey ? <p className="settings-card__error">{t(errorKey)}</p> : null}
      </form>
    </Modal>
  );
}
