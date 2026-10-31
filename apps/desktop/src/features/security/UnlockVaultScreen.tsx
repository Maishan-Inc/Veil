import { useState, type ChangeEvent, type FormEvent, type ReactNode } from "react";
import { QRCodeSVG } from "@rc-component/qrcode";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useT } from "@veil/i18n";
import { Button, Checkbox, Spinner, TextField } from "@veil/ui";
import {
  AppError,
  beginTotpEnrollment,
  confirmTotpEnrollment,
  getVaultStatus,
  isTauriEnvironment,
  recoverVaultWithRecoveryPhrase,
  unlockVault,
  type SensitiveActionInput,
  type TotpEnrollment,
  type VaultStatus,
} from "../../api/client";
import logoUrl from "../../assets/veil-logo.png";
import { Icon } from "../../shell/Icon";

interface UnlockVaultScreenProps {
  onUnlocked: (status: VaultStatus) => void;
}

export function UnlockVaultScreen({ onUnlocked }: UnlockVaultScreenProps): ReactNode {
  const t = useT();
  const [recovering, setRecovering] = useState(false);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    setBusy(true);
    setErrorKey(null);
    try {
      const status = await unlockVault(password);
      setPassword("");
      onUnlocked(status);
    } catch (err) {
      const code = err instanceof AppError ? err.code : "security.verificationFailed";
      setErrorKey(`errors.${code}`);
    } finally {
      setBusy(false);
    }
  }

  function minimizeWindow(): void {
    if (isTauriEnvironment()) void getCurrentWindow().minimize();
  }

  function toggleMaximizeWindow(): void {
    if (isTauriEnvironment()) void getCurrentWindow().toggleMaximize();
  }

  function closeWindow(): void {
    if (isTauriEnvironment()) void getCurrentWindow().close();
  }

  if (recovering) {
    return (
      <RecoveryVaultScreen
        onCancel={() => setRecovering(false)}
        onRecovered={onUnlocked}
        onMinimize={minimizeWindow}
        onMaximize={toggleMaximizeWindow}
        onClose={closeWindow}
      />
    );
  }

  return (
    <div className="onboarding-shell vault-unlock">
      <div className="onboarding-chrome" data-tauri-drag-region>
        <div className="onboarding-chrome__brand" data-tauri-drag-region>
          <img src={logoUrl} alt="" aria-hidden="true" />
          <span data-tauri-drag-region>{t("app.name")}</span>
        </div>
        <div className="onboarding-chrome__controls">
          <button
            type="button"
            aria-label={t("window.minimize")}
            title={t("window.minimize")}
            onClick={minimizeWindow}
          >
            <Icon name="minus" size={17} />
          </button>
          <button
            type="button"
            aria-label={t("window.maximize")}
            title={t("window.maximize")}
            onClick={toggleMaximizeWindow}
          >
            <Icon name="square" size={15} />
          </button>
          <button
            type="button"
            className="onboarding-chrome__close"
            aria-label={t("window.close")}
            title={t("window.close")}
            onClick={closeWindow}
          >
            <Icon name="x" size={17} />
          </button>
        </div>
      </div>

      <form
        className="onboarding-panel vault-unlock__panel"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <section className="onboarding-step vault-unlock__content">
          <img className="vault-unlock__logo" src={logoUrl} alt="" aria-hidden="true" />
          <div className="vault-unlock__copy">
            <h1>{t("vault.unlock.title")}</h1>
            <p>{t("vault.unlock.body")}</p>
          </div>
          <TextField
            type="password"
            label={t("vault.unlock.password")}
            placeholder={t("vault.unlock.password.placeholder")}
            value={password}
            onChange={(event) => setPassword(event.currentTarget.value)}
            autoComplete="current-password"
            autoFocus
          />
          {errorKey ? <p className="onboarding-error">{t(errorKey)}</p> : null}
          <Button
            type="submit"
            variant="primary"
            size="lg"
            disabled={busy || password.trim().length === 0}
            aria-label={busy ? t("common.loading") : undefined}
          >
            {busy ? <Spinner size={18} label={t("common.loading")} /> : t("vault.unlock.action")}
          </Button>
          <Button type="button" variant="subtle" onClick={() => setRecovering(true)}>
            {t("vault.recovery.open")}
          </Button>
        </section>
      </form>
    </div>
  );
}

type RecoveryStep = "phrase" | "password" | "twoFactor" | "twoFactorVerify";

function RecoveryVaultScreen({
  onCancel,
  onRecovered,
  onMinimize,
  onMaximize,
  onClose,
}: {
  onCancel: () => void;
  onRecovered: (status: VaultStatus) => void;
  onMinimize: () => void;
  onMaximize: () => void;
  onClose: () => void;
}): ReactNode {
  const t = useT();
  const [step, setStep] = useState<RecoveryStep>("phrase");
  const [phraseText, setPhraseText] = useState("");
  const [skipPassword, setSkipPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [passwordVisible, setPasswordVisible] = useState(false);
  const [confirmPasswordVisible, setConfirmPasswordVisible] = useState(false);
  const [twoFactorEnabled, setTwoFactorEnabled] = useState(false);
  const [totpEnrollment, setTotpEnrollment] = useState<TotpEnrollment | null>(null);
  const [totpCode, setTotpCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  const phrase = parseRecoveryPhrase(phraseText);
  const passwordActive = !skipPassword;
  const verification: SensitiveActionInput = {
    password: passwordActive ? password : null,
    twoFactorCode: null,
  };

  async function recover(): Promise<void> {
    setBusy(true);
    setErrorKey(null);
    try {
      const status = await recoverVaultWithRecoveryPhrase(phrase, {
        encryptionMode: passwordActive ? "standard" : "none",
        password: passwordActive ? password : null,
        twoFactorEnabled,
      });
      if (!twoFactorEnabled) {
        onRecovered(status);
        return;
      }
      const enrollment = await beginTotpEnrollment(verification);
      setTotpEnrollment(enrollment);
      setStep("twoFactorVerify");
    } catch (err) {
      const code = err instanceof AppError ? err.code : "security.verificationFailed";
      setErrorKey(`errors.${code}`);
    } finally {
      setBusy(false);
    }
  }

  async function confirmTotp(): Promise<void> {
    if (!totpEnrollment) return;
    setBusy(true);
    setErrorKey(null);
    try {
      await confirmTotpEnrollment(totpEnrollment.secret, totpCode.trim(), verification);
      onRecovered(await getVaultStatus());
    } catch (err) {
      const code = err instanceof AppError ? err.code : "security.verificationFailed";
      setErrorKey(`errors.${code}`);
    } finally {
      setBusy(false);
    }
  }

  function canContinuePassword(): boolean {
    if (!passwordActive) return true;
    return password.length >= 8 && password === confirmPassword;
  }

  return (
    <div className="onboarding-shell vault-unlock">
      <div className="onboarding-chrome" data-tauri-drag-region>
        <div className="onboarding-chrome__brand" data-tauri-drag-region>
          <img src={logoUrl} alt="" aria-hidden="true" />
          <span data-tauri-drag-region>{t("app.name")}</span>
        </div>
        <div className="onboarding-chrome__controls">
          <button type="button" aria-label={t("window.minimize")} onClick={onMinimize}>
            <Icon name="minus" size={17} />
          </button>
          <button type="button" aria-label={t("window.maximize")} onClick={onMaximize}>
            <Icon name="square" size={15} />
          </button>
          <button
            type="button"
            className="onboarding-chrome__close"
            aria-label={t("window.close")}
            onClick={onClose}
          >
            <Icon name="x" size={17} />
          </button>
        </div>
      </div>

      <section className="onboarding-panel vault-recovery__panel">
        <div className="vault-recovery__header">
          <h1>{t("vault.recovery.title")}</h1>
          <p>{t("vault.recovery.body")}</p>
        </div>

        {step === "phrase" ? (
          <div className="vault-recovery__body">
            <label className="vault-recovery__field">
              <span>{t("vault.recovery.phrase")}</span>
              <textarea
                className="veil-input vault-recovery__textarea"
                value={phraseText}
                onChange={(event) => setPhraseText(event.currentTarget.value)}
                placeholder={t("vault.recovery.phrase.placeholder")}
                autoFocus
              />
            </label>
            <p className="onboarding-security-note">{t("vault.recovery.note")}</p>
          </div>
        ) : step === "password" ? (
          <div className="vault-recovery__body">
            <label className="onboarding-check">
              <Checkbox
                checked={skipPassword}
                onChange={(event) => setSkipPassword(event.currentTarget.checked)}
              />
              <span>{t("onboarding.password.noPassword")}</span>
            </label>
            {passwordActive ? (
              <>
                <RecoveryPasswordField
                  label={t("onboarding.password.password")}
                  visible={passwordVisible}
                  onToggleVisible={() => setPasswordVisible((visible) => !visible)}
                  value={password}
                  onChange={(event) => setPassword(event.currentTarget.value)}
                  autoComplete="new-password"
                />
                <RecoveryPasswordField
                  label={t("onboarding.password.confirm")}
                  visible={confirmPasswordVisible}
                  onToggleVisible={() => setConfirmPasswordVisible((visible) => !visible)}
                  value={confirmPassword}
                  onChange={(event) => setConfirmPassword(event.currentTarget.value)}
                  autoComplete="new-password"
                />
              </>
            ) : null}
          </div>
        ) : step === "twoFactor" ? (
          <div className="vault-recovery__body">
            <div className="onboarding-choice-grid onboarding-choice-grid--two">
              <button
                type="button"
                className="onboarding-choice"
                data-active={twoFactorEnabled}
                onClick={() => setTwoFactorEnabled(true)}
              >
                <strong>{t("onboarding.2fa.enable")}</strong>
                <span>{t("onboarding.2fa.enableBody")}</span>
              </button>
              <button
                type="button"
                className="onboarding-choice"
                data-active={!twoFactorEnabled}
                onClick={() => setTwoFactorEnabled(false)}
              >
                <strong>{t("onboarding.2fa.skip")}</strong>
                <span>{t("onboarding.2fa.skipBody")}</span>
              </button>
            </div>
          </div>
        ) : (
          <div className="vault-recovery__body">
            {totpEnrollment ? (
              <div className="onboarding-2fa">
                <div className="onboarding-2fa__qr" aria-label={t("onboarding.2fa.verifyTitle")}>
                  <QRCodeSVG value={totpEnrollment.otpauthUri} size={184} marginSize={2} />
                </div>
                <TextField
                  label={t("onboarding.2fa.secret")}
                  value={totpEnrollment.secret}
                  readOnly
                />
                <TextField
                  inputMode="numeric"
                  label={t("onboarding.2fa.code")}
                  placeholder={t("security.verify.2fa.placeholder")}
                  value={totpCode}
                  onChange={(event) =>
                    setTotpCode(event.currentTarget.value.replace(/\D/g, "").slice(0, 6))
                  }
                  autoComplete="one-time-code"
                />
              </div>
            ) : (
              <p className="onboarding-muted">
                <Spinner size={16} label={t("common.loading")} />
              </p>
            )}
          </div>
        )}

        {errorKey ? <p className="onboarding-error">{t(errorKey)}</p> : null}

        <div className="onboarding-actions">
          <Button
            disabled={busy}
            onClick={() => {
              if (step === "phrase") onCancel();
              else if (step === "password") setStep("phrase");
              else if (step === "twoFactor") setStep("password");
              else setStep("twoFactor");
            }}
          >
            {t("common.back")}
          </Button>
          {step === "phrase" ? (
            <Button
              variant="primary"
              disabled={phrase.length !== 12 || new Set(phrase).size !== 12}
              onClick={() => setStep("password")}
            >
              {t("common.next")}
            </Button>
          ) : step === "password" ? (
            <Button
              variant="primary"
              disabled={!canContinuePassword()}
              onClick={() => setStep("twoFactor")}
            >
              {t("common.next")}
            </Button>
          ) : step === "twoFactor" ? (
            <Button
              variant="primary"
              disabled={busy}
              onClick={() => void recover()}
              aria-label={busy ? t("common.loading") : undefined}
            >
              {busy ? <Spinner size={16} label={t("common.loading")} /> : t("common.next")}
            </Button>
          ) : (
            <Button
              variant="primary"
              disabled={busy || totpCode.trim().length !== 6}
              onClick={() => void confirmTotp()}
              aria-label={busy ? t("common.loading") : undefined}
            >
              {busy ? <Spinner size={16} label={t("common.loading")} /> : t("common.confirm")}
            </Button>
          )}
        </div>
      </section>
    </div>
  );
}

function parseRecoveryPhrase(value: string): string[] {
  return value
    .split(/[\s,，]+/)
    .map((part) => part.replace(/-/g, "").trim().toUpperCase())
    .filter(Boolean);
}

function RecoveryPasswordField({
  label,
  value,
  visible,
  autoComplete,
  onToggleVisible,
  onChange,
}: {
  label: string;
  value: string;
  visible: boolean;
  autoComplete?: string;
  onToggleVisible: () => void;
  onChange: (event: ChangeEvent<HTMLInputElement>) => void;
}): ReactNode {
  const t = useT();
  return (
    <label className="veil-field onboarding-password-field">
      <span className="veil-field__label">{label}</span>
      <span className="onboarding-password-field__control">
        <input
          className="veil-input"
          type={visible ? "text" : "password"}
          value={value}
          onChange={onChange}
          autoComplete={autoComplete}
        />
        <button
          type="button"
          className="onboarding-password-field__toggle"
          onClick={onToggleVisible}
          aria-label={t(visible ? "common.hidePassword" : "common.showPassword")}
          title={t(visible ? "common.hidePassword" : "common.showPassword")}
        >
          <Icon name="eye" size={15} />
        </button>
      </span>
    </label>
  );
}
