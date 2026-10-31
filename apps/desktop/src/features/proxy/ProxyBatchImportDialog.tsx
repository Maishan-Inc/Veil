import { useMemo, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Banner, Button, Modal } from "@veil/ui";
import { AppError, createProxy } from "../../api/client";
import { parseProxyImportText } from "./proxyImport";

export interface ProxyBatchImportDialogProps {
  open: boolean;
  onClose: () => void;
  onImported: (count: number) => void | Promise<void>;
}

export function ProxyBatchImportDialog({
  open,
  onClose,
  onImported,
}: ProxyBatchImportDialogProps): ReactNode {
  const t = useT();
  const [text, setText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const parsed = useMemo(() => parseProxyImportText(text), [text]);

  async function handleImport(): Promise<void> {
    if (submitting || parsed.valid.length === 0) return;
    setSubmitting(true);
    setErrorCode(null);
    let imported = 0;
    let firstErrorCode: string | null = null;
    const failedRaw: string[] = parsed.invalid.map((line) => line.raw);

    for (const line of parsed.valid) {
      try {
        await createProxy(line.input);
        imported += 1;
      } catch (err) {
        failedRaw.push(line.raw);
        if (!firstErrorCode) {
          firstErrorCode = err instanceof AppError ? err.code : "app.unknown";
        }
      }
    }

    setErrorCode(firstErrorCode);
    setSubmitting(false);
    if (failedRaw.length > 0) {
      setText(failedRaw.join("\n"));
    } else {
      setText("");
    }
    if (imported > 0) {
      await onImported(imported);
    }
    if (failedRaw.length === 0 && imported > 0) {
      handleClose();
    }
  }

  function handleClose(): void {
    if (submitting) return;
    setText("");
    setErrorCode(null);
    onClose();
  }

  const invalidPreview = parsed.invalid.slice(0, 4);

  return (
    <Modal
      open={open}
      title={t("proxy.batch.title")}
      onClose={handleClose}
      size="lg"
      dismissOnBackdrop={!submitting}
      dismissOnEscape={!submitting}
      footer={
        <>
          <Button variant="secondary" onClick={handleClose} disabled={submitting}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            onClick={() => void handleImport()}
            disabled={submitting || parsed.valid.length === 0}
          >
            {t("proxy.batch.import")}
          </Button>
        </>
      }
    >
      <div className="proxy-batch">
        {errorCode ? (
          <Banner tone="warning" title={t(`errors.${errorCode}`)}>
            {t("proxy.batch.partialFailure")}
          </Banner>
        ) : null}
        <label className="veil-field">
          <span className="veil-field__label">{t("proxy.batch.input")}</span>
          <textarea
            className="veil-input proxy-batch__textarea"
            value={text}
            onChange={(event) => setText(event.currentTarget.value)}
            placeholder={t("proxy.batch.placeholder")}
            autoFocus
            spellCheck={false}
          />
          <span className="veil-field__hint">{t("proxy.batch.hint")}</span>
        </label>
        <div className="proxy-batch__summary" role="status">
          <span>{t("proxy.batch.validCount", { count: parsed.valid.length })}</span>
          <span>{t("proxy.batch.invalidCount", { count: parsed.invalid.length })}</span>
        </div>
        {invalidPreview.length > 0 ? (
          <div className="proxy-batch__invalid">
            <span className="proxy-batch__invalid-title">{t("proxy.batch.invalidTitle")}</span>
            {invalidPreview.map((line) => (
              <code key={`${line.lineNumber}-${line.raw}`}>
                {line.lineNumber}: {line.raw}
              </code>
            ))}
          </div>
        ) : null}
      </div>
    </Modal>
  );
}
