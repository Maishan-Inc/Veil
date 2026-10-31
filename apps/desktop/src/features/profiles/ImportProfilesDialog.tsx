import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Button, Modal, Select, Spinner, TextField } from "@veil/ui";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  AppError,
  importProfiles,
  type ImportConflict,
  type ImportProfilesResult,
} from "../../api/client";

interface ImportProfilesDialogProps {
  open: boolean;
  onClose: () => void;
  /** Called after a successful import so the page can refresh. */
  onImported: (result: ImportProfilesResult) => void;
}

/**
 * Import profiles from a Veil export file. The file is picked through the native dialog,
 * decrypted with the export passphrase when it is an encrypted (v2) export, and every
 * profile lands as a new row; name collisions follow the chosen conflict policy.
 */
export function ImportProfilesDialog({
  open,
  onClose,
  onImported,
}: ImportProfilesDialogProps): ReactNode {
  const t = useT();
  const [path, setPath] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [onConflict, setOnConflict] = useState<ImportConflict>("rename");
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);
  const [result, setResult] = useState<ImportProfilesResult | null>(null);

  useEffect(() => {
    if (!open) return;
    setPath("");
    setPassphrase("");
    setOnConflict("rename");
    setErrorKey(null);
    setResult(null);
  }, [open]);

  async function chooseFile(): Promise<void> {
    const selected = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Veil profile export", extensions: ["json"] }],
    });
    if (typeof selected === "string") setPath(selected);
  }

  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!path || busy) return;
    setBusy(true);
    setErrorKey(null);
    try {
      const importResult = await importProfiles({
        path,
        passphrase: passphrase.length > 0 ? passphrase : null,
        onConflict,
      });
      setResult(importResult);
      onImported(importResult);
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
      title={t("profiles.import.title")}
      onClose={onClose}
      size="sm"
      footer={
        <>
          <Button variant="subtle" onClick={onClose} disabled={busy}>
            {result ? t("common.close") : t("common.cancel")}
          </Button>
          <Button
            type="submit"
            form="veil-import-profiles-form"
            variant="primary"
            disabled={busy || path.length === 0 || Boolean(result)}
            aria-label={busy ? t("common.loading") : undefined}
          >
            {busy ? <Spinner size={16} label={t("common.loading")} /> : t("profiles.import.action")}
          </Button>
        </>
      }
    >
      <form
        id="veil-import-profiles-form"
        className="veil-form"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <div className="profiles-import-dialog__picker">
          <TextField
            label={t("profiles.import.file")}
            placeholder={t("profiles.import.file.placeholder")}
            value={path}
            onChange={(event) => setPath(event.currentTarget.value)}
            readOnly
          />
          <Button variant="secondary" onClick={() => void chooseFile()}>
            {t("profiles.import.file.choose")}
          </Button>
        </div>
        <TextField
          type="password"
          label={t("profiles.import.passphrase")}
          placeholder={t("profiles.import.passphrase.placeholder")}
          hint={t("profiles.import.passphrase.hint")}
          value={passphrase}
          onChange={(event) => setPassphrase(event.currentTarget.value)}
          autoComplete="off"
        />
        <Select
          label={t("profiles.import.conflict")}
          value={onConflict}
          onChange={(event) => setOnConflict(event.currentTarget.value as ImportConflict)}
          options={[
            { value: "rename", label: t("profiles.import.conflict.rename") },
            { value: "skip", label: t("profiles.import.conflict.skip") },
          ]}
        />
        {result ? (
          <p className="profiles-import-dialog__summary" role="status">
            {t("profiles.import.success", {
              imported: result.imported,
              renamed: result.renamed,
              skipped: result.skipped,
              failed: result.failed,
            })}
          </p>
        ) : null}
        {errorKey ? <p className="settings-card__error">{t(errorKey)}</p> : null}
      </form>
    </Modal>
  );
}
