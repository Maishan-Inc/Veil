import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type ClipboardEvent,
  type ReactNode,
} from "react";
import { QRCodeSVG } from "@rc-component/qrcode";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useT } from "@veil/i18n";
import { Button, Checkbox, Spinner, TextField, cx } from "@veil/ui";
import {
  AppError,
  beginTotpEnrollment,
  completeOnboarding,
  confirmTotpEnrollment,
  generateBackendRecoveryPhrase,
  isTauriEnvironment,
  saveRecoveryPhraseTxt,
  type EncryptionMode,
  type SensitiveActionInput,
  type TotpEnrollment,
} from "../../api/client";
import logoUrl from "../../assets/veil-logo.png";
import { Icon } from "../../shell/Icon";
import {
  generateRecoveryPhrase,
  pickVerificationIndexes,
  progressActive,
  recoveryPartInputValue,
  recoveryVerifyPartsFromPaste,
  verifyRecoveryPhrase,
  type OnboardingStep,
  type ProgressStepId,
} from "./onboardingHelpers";

interface OnboardingWizardProps {
  onComplete: () => void;
}

type CopyStatus = "idle" | "copied" | "failed";

const agreementParagraphs = Array.from({ length: 12 }, (_value, index) => `p${index + 1}`);

export function OnboardingWizard({ onComplete }: OnboardingWizardProps): ReactNode {
  const t = useT();
  const [step, setStep] = useState<OnboardingStep>("splash");
  const [agreementReady, setAgreementReady] = useState(false);
  const [encryptionMode, setEncryptionMode] = useState<EncryptionMode>("none");
  const [skipPassword, setSkipPassword] = useState(true);
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [passwordVisible, setPasswordVisible] = useState(false);
  const [confirmPasswordVisible, setConfirmPasswordVisible] = useState(false);
  const [passwordError, setPasswordError] = useState<string | null>(null);
  const [twoFactorEnabled, setTwoFactorEnabled] = useState(false);
  const [twoFactorEnrolled, setTwoFactorEnrolled] = useState(false);
  const [totpEnrollment, setTotpEnrollment] = useState<TotpEnrollment | null>(null);
  const [totpCode, setTotpCode] = useState("");
  const [totpErrorKey, setTotpErrorKey] = useState<string | null>(null);
  const [totpBusy, setTotpBusy] = useState(false);
  const [mnemonicEnabled, setMnemonicEnabled] = useState(false);
  const [recoveryPhrase, setRecoveryPhrase] = useState<string[]>([]);
  const [recoveryBusy, setRecoveryBusy] = useState(false);
  const [recoveryErrorKey, setRecoveryErrorKey] = useState<string | null>(null);
  const [verifyParts, setVerifyParts] = useState<Record<number, string>>({});
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<CopyStatus>("idle");
  const [finishErrorKey, setFinishErrorKey] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const agreementRef = useRef<HTMLDivElement>(null);

  const steps = [
    ["agreement", t("onboarding.step.agreement")],
    ["security", t("onboarding.step.security")],
    ["recovery", t("onboarding.step.recovery")],
    ["done", t("onboarding.step.done")],
  ] as const satisfies readonly (readonly [ProgressStepId, string])[];

  const passwordStrength = scorePassword(password);
  const mustUsePassword = encryptionMode === "standard";
  const passwordActive = mustUsePassword || !skipPassword;
  const verificationIndexes = useMemo(
    () => pickVerificationIndexes(recoveryPhrase.length),
    [recoveryPhrase.length],
  );
  const recoveryPhraseText = recoveryPhrase.join(" ");

  useEffect(() => {
    if (step !== "agreement") return;
    const id = window.setTimeout(() => {
      const el = agreementRef.current;
      if (el && el.scrollHeight <= el.clientHeight + 8) {
        setAgreementReady(true);
      }
    }, 0);
    return () => window.clearTimeout(id);
  }, [step]);

  useEffect(() => {
    if (step !== "mnemonic" || recoveryPhrase.length > 0 || recoveryBusy) return;
    void prepareRecoveryPhrase();
  }, [recoveryBusy, recoveryPhrase.length, step]);

  function selectEncryptionMode(mode: EncryptionMode): void {
    setEncryptionMode(mode);
    if (mode === "standard") {
      setSkipPassword(false);
    }
  }

  function goPassword(): void {
    setSkipPassword(encryptionMode === "none");
    setPasswordError(null);
    setStep("password");
  }

  function validatePasswordStep(): void {
    if (!passwordActive) {
      setPasswordError(null);
      setStep("twoFactor");
      return;
    }
    if (password.length < 8) {
      setPasswordError(t("onboarding.password.minLength"));
      return;
    }
    if (password !== confirmPassword) {
      setPasswordError(t("onboarding.password.mismatch"));
      return;
    }
    setPasswordError(null);
    setStep("twoFactor");
  }

  async function handleTwoFactorNext(): Promise<void> {
    if (!twoFactorEnabled) {
      resetTotpEnrollment();
      setTwoFactorEnrolled(false);
      setStep("mnemonic");
      return;
    }

    setStep("twoFactorVerify");
    await startTotpEnrollment();
  }

  async function startTotpEnrollment(): Promise<void> {
    setTotpBusy(true);
    setTotpErrorKey(null);
    setTotpEnrollment(null);
    setTotpCode("");
    try {
      const enrollment = await beginTotpEnrollment(sensitiveInput());
      setTotpEnrollment(enrollment);
    } catch (err) {
      setTotpErrorKey(toErrorKey(err, "onboarding.2fa.beginError"));
    } finally {
      setTotpBusy(false);
    }
  }

  async function confirmTotp(): Promise<void> {
    if (!totpEnrollment) return;
    const code = totpCode.trim();
    if (!/^\d{6}$/.test(code)) {
      setTotpErrorKey("onboarding.2fa.codeRequired");
      return;
    }

    setTotpBusy(true);
    setTotpErrorKey(null);
    try {
      await confirmTotpEnrollment(totpEnrollment.secret, code, sensitiveInput());
      setTwoFactorEnrolled(true);
      setStep("mnemonic");
    } catch (err) {
      setTotpErrorKey(toErrorKey(err, "onboarding.2fa.confirmError"));
    } finally {
      setTotpBusy(false);
    }
  }

  async function prepareRecoveryPhrase(): Promise<void> {
    setRecoveryBusy(true);
    setRecoveryErrorKey(null);
    const phrase = isTauriEnvironment()
      ? await generateBackendRecoveryPhrase()
          .then((output) => output.phrase)
          .catch((err) => {
            setRecoveryErrorKey(toErrorKey(err, "onboarding.mnemonic.generateError"));
            return [];
          })
      : generateRecoveryPhrase();
    if (phrase.length === 0) {
      setRecoveryBusy(false);
      return;
    }
    const indexes = pickVerificationIndexes(phrase.length);
    setRecoveryPhrase(phrase);
    setMnemonicEnabled(true);
    setVerifyParts(Object.fromEntries(indexes.map((index) => [index, ""])));
    setVerifyError(null);
    setCopyStatus("idle");
    setRecoveryBusy(false);
  }

  async function copyRecoveryPhrase(): Promise<void> {
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("clipboard unavailable");
      }
      await navigator.clipboard.writeText(recoveryPhraseText);
      setCopyStatus("copied");
    } catch {
      setCopyStatus("failed");
    }
  }

  async function downloadRecoveryPhrase(): Promise<void> {
    const createdAt = new Date().toISOString();
    const fileName = `veil-recovery-phrase-${createdAt.replace(/[:.]/g, "-")}.txt`;
    const content = [
      t("onboarding.mnemonic.exportTitle"),
      `${t("onboarding.mnemonic.exportCreatedAt")}: ${createdAt}`,
      "",
      t("onboarding.mnemonic.exportWarning"),
      "",
      recoveryPhraseText,
      "",
    ].join("\n");
    if (isTauriEnvironment()) {
      try {
        const path = await saveDialog({
          defaultPath: fileName,
          filters: [{ name: "Text", extensions: ["txt"] }],
        });
        if (!path) return;
        await saveRecoveryPhraseTxt(path, content);
      } catch {
        // The user can retry from the same screen; do not expose raw filesystem errors.
      }
      return;
    }

    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = fileName;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  }

  function skipRecoveryPhrase(): void {
    setMnemonicEnabled(false);
    setRecoveryPhrase([]);
    setVerifyParts({});
    setVerifyError(null);
    setCopyStatus("idle");
    setStep("done");
  }

  function verifyPhrase(): void {
    if (!verifyRecoveryPhrase(recoveryPhrase, verificationIndexes, verifyParts)) {
      setVerifyError(t("onboarding.mnemonic.verifyWrong"));
      return;
    }
    setVerifyError(null);
    setStep("done");
  }

  function pasteVerifyPhrase(index: number, event: ClipboardEvent<HTMLInputElement>): void {
    event.preventDefault();
    const pastedText = event.clipboardData.getData("text");
    setVerifyParts((current) =>
      recoveryVerifyPartsFromPaste(recoveryPhrase, verificationIndexes, current, index, pastedText),
    );
  }

  async function finish(): Promise<void> {
    setBusy(true);
    setFinishErrorKey(null);
    try {
      await completeOnboarding({
        encryptionMode,
        password: passwordActive ? password : null,
        twoFactorEnabled: twoFactorEnabled && twoFactorEnrolled,
        mnemonicEnabled,
      });
      setPassword("");
      setConfirmPassword("");
      resetTotpEnrollment();
      setRecoveryPhrase([]);
      onComplete();
    } catch (err) {
      setFinishErrorKey(toErrorKey(err, "errors.app.unknown"));
    } finally {
      setBusy(false);
    }
  }

  function sensitiveInput(): SensitiveActionInput {
    return {
      password: passwordActive ? password : null,
      twoFactorCode: null,
    };
  }

  function resetTotpEnrollment(): void {
    setTotpEnrollment(null);
    setTotpCode("");
    setTotpErrorKey(null);
    setTotpBusy(false);
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

  return (
    <div className="onboarding-shell">
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

      <div className="onboarding-panel onboarding-panel--wizard">
        <div className="onboarding-progress" aria-hidden={step === "splash"}>
          {steps.map(([id, label]) => (
            <span
              key={id}
              className="onboarding-progress__step"
              data-active={progressActive(step, id)}
            >
              {label}
            </span>
          ))}
        </div>

        {step === "splash" ? (
          <section className="onboarding-step onboarding-splash">
            <div className="onboarding-step__body">
              <img className="onboarding-splash__logo" src={logoUrl} alt="" aria-hidden="true" />
              <h1>{t("onboarding.splash.title")}</h1>
              <p>{t("onboarding.splash.subtitle")}</p>
              <Button variant="primary" size="lg" onClick={() => setStep("agreement")}>
                {t("onboarding.splash.start")}
              </Button>
            </div>
          </section>
        ) : step === "agreement" ? (
          <StepFrame
            title={t("onboarding.agreement.title")}
            actions={
              <Button
                variant="primary"
                size="lg"
                disabled={!agreementReady}
                onClick={() => setStep("encryption")}
              >
                {t("onboarding.agreement.accept")}
              </Button>
            }
          >
            <div
              ref={agreementRef}
              className="onboarding-agreement"
              onScroll={(event) => {
                const el = event.currentTarget;
                if (el.scrollTop + el.clientHeight >= el.scrollHeight - 8) {
                  setAgreementReady(true);
                }
              }}
            >
              {agreementParagraphs.map((key) => (
                <p key={key}>{t(`onboarding.agreement.${key}`)}</p>
              ))}
            </div>
            <p className="onboarding-hint">
              {agreementReady ? "" : t("onboarding.agreement.scrollHint")}
            </p>
          </StepFrame>
        ) : step === "encryption" ? (
          <StepFrame
            title={t("onboarding.encryption.title")}
            actions={
              <>
                <Button onClick={() => setStep("agreement")}>{t("common.back")}</Button>
                <Button variant="primary" onClick={goPassword}>
                  {t("common.next")}
                </Button>
              </>
            }
          >
            <div className="onboarding-choice-grid">
              <Choice
                active={encryptionMode === "none"}
                title={t("onboarding.encryption.none.title")}
                body={t("onboarding.encryption.none.body")}
                onClick={() => selectEncryptionMode("none")}
              />
              <Choice
                active={encryptionMode === "standard"}
                title={t("onboarding.encryption.standard.title")}
                body={t("onboarding.encryption.standard.body")}
                onClick={() => selectEncryptionMode("standard")}
              />
              <Choice
                disabled
                title={t("onboarding.encryption.advanced.title")}
                body={t("onboarding.encryption.advanced.body")}
                onClick={() => undefined}
              />
            </div>
            <p className="onboarding-security-note">
              {t(`onboarding.encryption.${encryptionMode}.note`)}
            </p>
          </StepFrame>
        ) : step === "password" ? (
          <StepFrame
            title={t("onboarding.password.title")}
            description={
              mustUsePassword
                ? t("onboarding.password.requiredBody")
                : t("onboarding.password.optionalBody")
            }
            actions={
              <>
                <Button onClick={() => setStep("encryption")}>{t("common.back")}</Button>
                <Button variant="primary" onClick={validatePasswordStep}>
                  {t("common.next")}
                </Button>
              </>
            }
          >
            {!mustUsePassword ? (
              <label className="onboarding-check">
                <Checkbox
                  checked={skipPassword}
                  onChange={(event) => setSkipPassword(event.currentTarget.checked)}
                />
                <span>{t("onboarding.password.noPassword")}</span>
              </label>
            ) : null}
            {passwordActive ? (
              <>
                <PasswordField
                  label={t("onboarding.password.password")}
                  visible={passwordVisible}
                  onToggleVisible={() => setPasswordVisible((visible) => !visible)}
                  value={password}
                  onChange={(event) => setPassword(event.currentTarget.value)}
                  autoComplete="new-password"
                />
                <PasswordField
                  label={t("onboarding.password.confirm")}
                  visible={confirmPasswordVisible}
                  onToggleVisible={() => setConfirmPasswordVisible((visible) => !visible)}
                  value={confirmPassword}
                  onChange={(event) => setConfirmPassword(event.currentTarget.value)}
                  autoComplete="new-password"
                />
                <div className="onboarding-strength" data-strength={passwordStrength}>
                  <span />
                  <span />
                  <span />
                  <em>
                    {passwordStrength === 0
                      ? t("onboarding.password.weak")
                      : passwordStrength === 1
                        ? t("onboarding.password.medium")
                        : t("onboarding.password.strong")}
                  </em>
                </div>
              </>
            ) : null}
            <p className="onboarding-security-note">{t("onboarding.password.warning")}</p>
            {passwordError ? <p className="onboarding-error">{passwordError}</p> : null}
          </StepFrame>
        ) : step === "twoFactor" ? (
          <StepFrame
            title={t("onboarding.2fa.title")}
            description={t("onboarding.2fa.body")}
            actions={
              <>
                <Button onClick={() => setStep("password")}>{t("common.back")}</Button>
                <Button
                  variant="primary"
                  disabled={totpBusy}
                  onClick={() => void handleTwoFactorNext()}
                  aria-label={totpBusy ? t("common.loading") : undefined}
                >
                  {totpBusy ? <Spinner size={16} label={t("common.loading")} /> : t("common.next")}
                </Button>
              </>
            }
          >
            <div className="onboarding-choice-grid onboarding-choice-grid--two">
              <Choice
                active={twoFactorEnabled}
                title={t("onboarding.2fa.enable")}
                body={t("onboarding.2fa.enableBody")}
                onClick={() => {
                  setTwoFactorEnabled(true);
                  setTwoFactorEnrolled(false);
                }}
              />
              <Choice
                active={!twoFactorEnabled}
                title={t("onboarding.2fa.skip")}
                body={t("onboarding.2fa.skipBody")}
                onClick={() => {
                  setTwoFactorEnabled(false);
                  setTwoFactorEnrolled(false);
                  resetTotpEnrollment();
                }}
              />
            </div>
          </StepFrame>
        ) : step === "twoFactorVerify" ? (
          <StepFrame
            title={t("onboarding.2fa.verifyTitle")}
            description={t("onboarding.2fa.verifyBody")}
            actions={
              <>
                <Button
                  disabled={totpBusy}
                  onClick={() => {
                    resetTotpEnrollment();
                    setStep("twoFactor");
                  }}
                >
                  {t("common.back")}
                </Button>
                {!totpEnrollment ? (
                  <Button
                    variant="primary"
                    disabled={totpBusy}
                    onClick={() => void startTotpEnrollment()}
                    aria-label={totpBusy ? t("common.loading") : undefined}
                  >
                    {totpBusy ? (
                      <Spinner size={16} label={t("common.loading")} />
                    ) : (
                      t("common.retry")
                    )}
                  </Button>
                ) : (
                  <Button
                    variant="primary"
                    disabled={totpBusy || totpCode.trim().length !== 6}
                    onClick={() => void confirmTotp()}
                    aria-label={totpBusy ? t("common.loading") : undefined}
                  >
                    {totpBusy ? (
                      <Spinner size={16} label={t("common.loading")} />
                    ) : (
                      t("common.confirm")
                    )}
                  </Button>
                )}
              </>
            }
          >
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
                {totpBusy ? (
                  <Spinner size={16} label={t("common.loading")} />
                ) : (
                  t("onboarding.2fa.waiting")
                )}
              </p>
            )}
            {totpErrorKey ? <p className="onboarding-error">{t(totpErrorKey)}</p> : null}
          </StepFrame>
        ) : step === "mnemonic" ? (
          <StepFrame
            title={t("onboarding.mnemonic.title")}
            description={t("onboarding.mnemonic.body")}
            actions={
              <>
                <Button onClick={() => setStep("twoFactor")}>{t("common.back")}</Button>
                <Button onClick={skipRecoveryPhrase}>{t("onboarding.mnemonic.skip")}</Button>
                <Button
                  variant="primary"
                  disabled={recoveryBusy || recoveryPhrase.length === 0}
                  onClick={() => setStep("mnemonicVerify")}
                  aria-label={recoveryBusy ? t("common.loading") : undefined}
                >
                  {recoveryBusy ? (
                    <Spinner size={16} label={t("common.loading")} />
                  ) : (
                    t("common.next")
                  )}
                </Button>
              </>
            }
          >
            {recoveryPhrase.length > 0 ? (
              <>
                <h3 className="onboarding-subtitle">{t("onboarding.mnemonic.showTitle")}</h3>
                <div className="onboarding-words">
                  {recoveryPhrase.map((part, index) => (
                    <span className="onboarding-word-entry" key={`${part}-${index}`}>
                      <b>{index + 1}</b>
                      {part}
                    </span>
                  ))}
                </div>
                <div className="onboarding-inline-actions">
                  <Button size="sm" onClick={() => void copyRecoveryPhrase()}>
                    <Icon name="copy" size={15} />
                    {t("onboarding.mnemonic.copy")}
                  </Button>
                  <Button size="sm" onClick={() => void downloadRecoveryPhrase()}>
                    <Icon name="export" size={15} />
                    {t("onboarding.mnemonic.download")}
                  </Button>
                  <span
                    className={cx("onboarding-copy-status", copyStatus === "failed" && "is-error")}
                  >
                    {copyStatus === "copied"
                      ? t("onboarding.mnemonic.copySuccess")
                      : copyStatus === "failed"
                        ? t("onboarding.mnemonic.copyFailed")
                        : ""}
                  </span>
                </div>
                <p className="onboarding-security-note">{t("onboarding.mnemonic.showBody")}</p>
              </>
            ) : recoveryBusy ? (
              <p className="onboarding-muted">
                <Spinner size={16} label={t("common.loading")} />
              </p>
            ) : (
              <p className="onboarding-error">
                {t(recoveryErrorKey ?? "onboarding.mnemonic.generateError")}
              </p>
            )}
          </StepFrame>
        ) : step === "mnemonicVerify" ? (
          <StepFrame
            title={t("onboarding.mnemonic.verifyTitle")}
            description={t("onboarding.mnemonic.verifyBody")}
            actions={
              <>
                <Button onClick={() => setStep("mnemonic")}>{t("common.back")}</Button>
                <Button variant="primary" onClick={verifyPhrase}>
                  {t("common.confirm")}
                </Button>
              </>
            }
          >
            <div className="onboarding-words onboarding-words--verify">
              {recoveryPhrase.map((part, index) =>
                verificationIndexes.includes(index) ? (
                  <label
                    className="onboarding-word-entry onboarding-word-entry--input"
                    key={`verify-${index}`}
                  >
                    <span>{t("onboarding.mnemonic.segment", { n: index + 1 })}</span>
                    <input
                      className="veil-input"
                      value={verifyParts[index] ?? ""}
                      onChange={(event) => {
                        const value = recoveryPartInputValue(event.currentTarget.value);
                        setVerifyParts((current) => ({
                          ...current,
                          [index]: value,
                        }));
                      }}
                      onPaste={(event) => pasteVerifyPhrase(index, event)}
                      maxLength={4}
                      autoComplete="off"
                    />
                  </label>
                ) : (
                  <span className="onboarding-word-entry" key={`${part}-${index}`}>
                    <b>{index + 1}</b>
                    {part}
                  </span>
                ),
              )}
            </div>
            {verifyError ? <p className="onboarding-error">{verifyError}</p> : null}
          </StepFrame>
        ) : (
          <section className="onboarding-step onboarding-done">
            <div className="onboarding-step__body">
              <div className="onboarding-done__mark">
                <Icon name="check" size={30} />
              </div>
              <h2>{t("onboarding.done.title")}</h2>
              <p className="onboarding-muted">{t("onboarding.done.body")}</p>
              <p className="onboarding-security-note">{t("onboarding.done.securityNote")}</p>
              {finishErrorKey ? <p className="onboarding-error">{t(finishErrorKey)}</p> : null}
              <Button
                variant="primary"
                size="lg"
                disabled={busy}
                onClick={() => void finish()}
                aria-label={busy ? t("common.loading") : undefined}
              >
                {busy ? (
                  <Spinner size={18} label={t("common.loading")} />
                ) : (
                  t("onboarding.done.open")
                )}
              </Button>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function StepFrame({
  title,
  description,
  children,
  actions,
}: {
  title: string;
  description?: string;
  children: ReactNode;
  actions: ReactNode;
}): ReactNode {
  return (
    <section className="onboarding-step">
      <div className="onboarding-step__header">
        <h2>{title}</h2>
        {description ? <p className="onboarding-muted">{description}</p> : null}
      </div>
      <div className="onboarding-step__body">{children}</div>
      <div className="onboarding-actions">{actions}</div>
    </section>
  );
}

function Choice({
  active = false,
  disabled = false,
  title,
  body,
  onClick,
}: {
  active?: boolean;
  disabled?: boolean;
  title: string;
  body: string;
  onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className={cx("onboarding-choice")}
      data-active={active}
      disabled={disabled}
      onClick={onClick}
    >
      <strong>{title}</strong>
      <span>{body}</span>
    </button>
  );
}

interface PasswordFieldProps {
  label: string;
  value: string;
  visible: boolean;
  autoComplete?: string;
  onToggleVisible: () => void;
  onChange: (event: ChangeEvent<HTMLInputElement>) => void;
}

function PasswordField({
  label,
  value,
  visible,
  autoComplete,
  onToggleVisible,
  onChange,
}: PasswordFieldProps): ReactNode {
  const t = useT();
  const labelId = useId();
  return (
    <label className="veil-field onboarding-password-field">
      <span className="veil-field__label" id={labelId}>
        {label}
      </span>
      <span className="onboarding-password-field__control">
        <input
          className="veil-input"
          type={visible ? "text" : "password"}
          value={value}
          onChange={onChange}
          autoComplete={autoComplete}
          aria-labelledby={labelId}
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

function scorePassword(value: string): 0 | 1 | 2 {
  if (value.length < 8) return 0;
  const classes = [/[a-z]/, /[A-Z]/, /[0-9]/, /[^a-zA-Z0-9]/].filter((regex) =>
    regex.test(value),
  ).length;
  return value.length >= 12 && classes >= 3 ? 2 : 1;
}

function toErrorKey(err: unknown, fallback: string): string {
  return err instanceof AppError ? `errors.${err.code}` : fallback;
}
