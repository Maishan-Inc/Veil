import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Banner, Button, Modal, Spinner, TextField } from "@veil/ui";
import {
  AppError,
  exportProfiles,
  getOnboardingStatus,
  type OnboardingStatus,
  type SensitiveActionInput,
} from "../../api/client";

interface ExportProfilesDialogProps {
  open: boolean;
  /** The profiles to export; the count is shown in the copy. */
  profileIds: string[];
  onClose: () => void;
  onExported: (result: { count: number; path: string }) => void;
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

/**
 * Export the selected profiles. The file is written encrypted (Argon2id + AES-256-GCM under
 * a user passphrase), so the dialog collects the passphrase and the sensitive-action
 * verification in one form.
 */
export function ExportProfilesDialog({
  open,
  profileIds,
  onClose,
  onExported,
}: ExportProfilesDialogProps): ReactNode {
  const t = useT();
  const [status, setStatus] = useState<OnboardingStatus | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setPassphrase("");
    setPassphraseConfirm("");
    setPassword("");
    setTwoFactorCode("");
    setErrorKey(null);
    setStatus(null);
    getOnboardingStatus()
      .then(setStatus)
      .catch(() => setStatus(FALLBACK_STATUS));
  }, [open]);

  const needsPassword = status?.passwordRequired ?? false;
  const needsTwoFactor = status?.twoFactorEnabled ?? false;
  const passphraseMismatch =
    passphrase.length > 0 && passphraseConfirm.length > 0 && passphrase !== passphraseConfirm;
  const canSubmit =
    !busy &&
    profileIds.length > 0 &&
    passphrase.length > 0 &&
    !passphraseMismatch &&
    (!needsPassword || password.trim().length > 0) &&
    (!needsTwoFactor || twoFactorCode.trim().length > 0);

  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    setBusy(true);
    setErrorKey(null);
    const verification: SensitiveActionInput = {
      password: needsPassword ? password : null,
      twoFactorCode: needsTwoFactor ? twoFactorCode : null,
    };
    try {
      const result = await exportProfiles(profileIds, passphrase, verification);
      onExported(result);
      onClose();
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setErrorKey(`errors.${code}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      title={t("profiles.export.title")}
      onClose={onClose}
      size="sm"
      footer={
        <>
          <Button variant="subtle" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button
            type="submit"
            form="veil-export-profiles-form"
            variant="primary"
            disabled={!canSubmit}
            aria-label={busy ? t("common.loading") : undefined}
          >
            {busy ? <Spinner size={16} label={t("common.loading")} /> : t("profiles.export.action")}
          </Button>
        </>
      }
    >
      <form
        id="veil-export-profiles-form"
        className="veil-form"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <p className="sensitive-action__body">
          {t("profiles.export.body", { count: profileIds.length })}
        </p>
        <TextField
          type="password"
          label={t("profiles.export.passphrase")}
          placeholder={t("profiles.export.passphrase.placeholder")}
          value={passphrase}
          onChange={(event) => setPassphrase(event.currentTarget.value)}
          autoComplete="new-password"
        />
        <TextField
          type="password"
          label={t("profiles.export.passphrase.confirm")}
          placeholder={t("profiles.export.passphrase.confirm.placeholder")}
          value={passphraseConfirm}
          onChange={(event) => setPassphraseConfirm(event.currentTarget.value)}
          autoComplete="new-password"
        />
        {passphraseMismatch ? (
          <Banner tone="warning">{t("profiles.export.passphrase.mismatch")}</Banner>
        ) : null}
        <Banner tone="info">{t("profiles.export.passphrase.hint")}</Banner>
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
