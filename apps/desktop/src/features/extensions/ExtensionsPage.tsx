import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { useT } from "@veil/i18n";
import type { AppErrorCode, BrowserExtension, Profile } from "@veil/shared";
import { extensionSourceLabelKey } from "@veil/shared";
import { Badge, Banner, Button, Checkbox, ConfirmDialog, Modal, TextField } from "@veil/ui";
import {
  AppError,
  deleteExtension,
  importExtensionFromLocal,
  importExtensionFromStore,
  listExtensions,
  listProfiles,
  updateExtension,
} from "../../api/client";
import { Icon } from "../../shell/Icon";

type EditState = { kind: "closed" } | { kind: "edit"; extension: BrowserExtension };
type DeleteState = { kind: "closed" } | { kind: "delete"; extension: BrowserExtension };
type StorePreview = { id: string; name: string | null };

export function ExtensionsPage(): ReactNode {
  const t = useT();
  const [extensions, setExtensions] = useState<BrowserExtension[]>([]);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [storeUrl, setStoreUrl] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [errorCode, setErrorCode] = useState<AppErrorCode | null>(null);
  const [edit, setEdit] = useState<EditState>({ kind: "closed" });
  const [confirm, setConfirm] = useState<DeleteState>({ kind: "closed" });
  const storeInputRef = useRef<HTMLInputElement | null>(null);

  const reload = useCallback(async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const [nextExtensions, nextProfiles] = await Promise.all([
        listExtensions(),
        listProfiles().catch(() => [] as Profile[]),
      ]);
      setExtensions(nextExtensions);
      setProfiles(nextProfiles);
    } finally {
      if (showLoading) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const profileNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const profile of profiles) map.set(profile.id, profile.name);
    return map;
  }, [profiles]);

  const storePreview = useMemo(() => parseChromeStoreId(storeUrl), [storeUrl]);

  function appErrorCode(err: unknown): AppErrorCode {
    return err instanceof AppError ? err.code : "app.unknown";
  }

  async function importFromStore(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const url = storeUrl.trim();
    if (!url) return;
    setBusy(true);
    setErrorCode(null);
    try {
      await importExtensionFromStore({ storeUrl: url, defaultEnabled: false, profileIds: [] });
      setStoreUrl("");
      await reload(false);
    } catch (err) {
      setErrorCode(appErrorCode(err));
    } finally {
      setBusy(false);
    }
  }

  async function importLocal(directory: boolean): Promise<void> {
    const selected = await openDialog(
      directory
        ? { multiple: false, directory: true }
        : {
            multiple: false,
            directory: false,
            filters: [{ name: "Extension ZIP", extensions: ["zip"] }],
          },
    );
    if (typeof selected !== "string") return;
    setBusy(true);
    setErrorCode(null);
    try {
      await importExtensionFromLocal({
        localPath: selected,
        defaultEnabled: false,
        profileIds: [],
      });
      await reload(false);
    } catch (err) {
      setErrorCode(appErrorCode(err));
    } finally {
      setBusy(false);
    }
  }

  async function saveExtension(input: {
    id: string;
    name: string;
    defaultEnabled: boolean;
    profileIds: string[];
  }): Promise<void> {
    setBusy(true);
    setErrorCode(null);
    try {
      await updateExtension(input.id, {
        name: input.name,
        defaultEnabled: input.defaultEnabled,
        profileIds: input.profileIds,
      });
      setEdit({ kind: "closed" });
      await reload(false);
    } catch (err) {
      setErrorCode(appErrorCode(err));
    } finally {
      setBusy(false);
    }
  }

  async function confirmDelete(): Promise<void> {
    if (confirm.kind !== "delete") return;
    setBusy(true);
    setErrorCode(null);
    try {
      await deleteExtension(confirm.extension.id);
      setConfirm({ kind: "closed" });
      await reload(false);
    } catch (err) {
      setErrorCode(appErrorCode(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="veil-section extensions-page">
      <header className="veil-section__header">
        <div>
          <h2 className="veil-section__title">{t("extensions.title")}</h2>
          <p className="extensions-page__subtitle">{t("extensions.subtitle")}</p>
        </div>
        <span className="veil-section__spacer" />
        <Button size="sm" variant="secondary" onClick={() => void reload()} disabled={loading}>
          <Icon name="refresh" size={14} />
          {t("toolbar.refresh")}
        </Button>
      </header>

      {errorCode ? (
        <Banner tone="warning" title={t(`errors.${errorCode}`)}>
          {t("extensions.error.body")}
        </Banner>
      ) : null}

      <section className="extensions-import-panel" aria-label={t("extensions.import.title")}>
        <form className="extensions-store-form" onSubmit={(event) => void importFromStore(event)}>
          <TextField
            ref={storeInputRef}
            label={t("extensions.storeUrl.label")}
            value={storeUrl}
            onChange={(event) => setStoreUrl(event.currentTarget.value)}
            placeholder={t("extensions.storeUrl.placeholder")}
          />
          <Button type="submit" variant="primary" disabled={busy || !storeUrl.trim()}>
            <Icon name="import" size={14} />
            {t("extensions.action.importStore")}
          </Button>
        </form>
        {storeUrl.trim() ? (
          <div className="extensions-store-preview" data-valid={Boolean(storePreview)}>
            <Icon name={storePreview ? "check" : "x"} size={14} />
            <div>
              <strong>
                {storePreview
                  ? storePreview.name
                    ? t("extensions.preview.nameValue", { name: storePreview.name })
                    : t("extensions.preview.name")
                  : t("extensions.preview.invalid")}
              </strong>
              <span>
                {storePreview
                  ? t("extensions.preview.id", { id: storePreview.id })
                  : t("extensions.preview.invalidBody")}
              </span>
            </div>
          </div>
        ) : null}
        <div className="extensions-local-actions">
          <Button variant="secondary" onClick={() => void importLocal(false)} disabled={busy}>
            <Icon name="folder" size={14} />
            {t("extensions.action.upload")}
          </Button>
          <Button variant="secondary" onClick={() => void importLocal(true)} disabled={busy}>
            <Icon name="folder" size={14} />
            {t("extensions.action.importFolder")}
          </Button>
        </div>
      </section>

      <section
        className="extensions-list"
        data-empty={!loading && extensions.length === 0}
        aria-label={t("extensions.installed.title")}
      >
        <div className="extensions-list__head">
          <h3 className="extensions-section-title">{t("extensions.installed.title")}</h3>
          <span>{t("extensions.installed.count", { count: extensions.length })}</span>
        </div>
        {!loading && extensions.length === 0 ? (
          <div className="extensions-empty">
            <Icon name="extensions" size={32} />
            <span className="extensions-empty__title">{t("extensions.empty.title")}</span>
            <span>{t("extensions.empty.body")}</span>
            <div className="extensions-empty__actions">
              <Button variant="primary" onClick={() => void importLocal(false)} disabled={busy}>
                <Icon name="folder" size={14} />
                {t("extensions.action.upload")}
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  storeInputRef.current?.focus();
                }}
              >
                <Icon name="import" size={14} />
                {t("extensions.action.importId")}
              </Button>
            </div>
          </div>
        ) : null}
        {extensions.map((extension) => (
          <article key={extension.id} className="extensions-row">
            <div className="extensions-row__main">
              <div className="extensions-row__title">
                <span className="extensions-row__name">{extension.name}</span>
                <Badge tone={extension.source === "chromeStore" ? "accent" : "neutral"}>
                  {t(extensionSourceLabelKey(extension.source))}
                </Badge>
                {extension.defaultEnabled ? (
                  <Badge tone="success">{t("extensions.default.badge")}</Badge>
                ) : null}
              </div>
              <div className="extensions-row__meta">
                <span>
                  {t("extensions.meta.version")}: {extension.version || t("common.none")}
                </span>
                <span>
                  {t("extensions.meta.assigned")}:{" "}
                  {t("extensions.assigned.count", { count: extension.profileIds.length })}
                </span>
              </div>
              <code className="extensions-row__path">{extension.installPath}</code>
              {extension.profileIds.length > 0 ? (
                <div className="extensions-row__profiles">
                  {extension.profileIds
                    .map((id) => profileNameById.get(id) ?? id)
                    .slice(0, 4)
                    .join(", ")}
                </div>
              ) : null}
            </div>
            <div className="extensions-row__actions">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setEdit({ kind: "edit", extension })}
              >
                <Icon name="edit" size={14} />
                {t("common.edit")}
              </Button>
              <Button
                variant="danger"
                size="sm"
                onClick={() => setConfirm({ kind: "delete", extension })}
              >
                <Icon name="trash" size={14} />
                {t("common.delete")}
              </Button>
            </div>
          </article>
        ))}
      </section>

      <ExtensionEditDialog
        open={edit.kind === "edit"}
        extension={edit.kind === "edit" ? edit.extension : null}
        profiles={profiles}
        busy={busy}
        onClose={() => setEdit({ kind: "closed" })}
        onSave={saveExtension}
      />

      <ConfirmDialog
        open={confirm.kind === "delete"}
        danger
        title={t("extensions.confirm.delete.title")}
        body={t("extensions.confirm.delete.body")}
        confirmLabel={t("common.delete")}
        cancelLabel={t("common.cancel")}
        onConfirm={() => void confirmDelete()}
        onClose={() => setConfirm({ kind: "closed" })}
      />
    </section>
  );
}

interface ExtensionEditDialogProps {
  open: boolean;
  extension: BrowserExtension | null;
  profiles: Profile[];
  busy: boolean;
  onClose: () => void;
  onSave: (input: {
    id: string;
    name: string;
    defaultEnabled: boolean;
    profileIds: string[];
  }) => Promise<void>;
}

function ExtensionEditDialog({
  open,
  extension,
  profiles,
  busy,
  onClose,
  onSave,
}: ExtensionEditDialogProps): ReactNode {
  const t = useT();
  const [name, setName] = useState("");
  const [defaultEnabled, setDefaultEnabled] = useState(false);
  const [profileIds, setProfileIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!extension) return;
    setName(extension.name);
    setDefaultEnabled(extension.defaultEnabled);
    setProfileIds(new Set(extension.profileIds));
  }, [extension]);

  if (!extension) return null;
  const currentExtension = extension;

  function toggleProfile(profileId: string): void {
    setProfileIds((prev) => {
      const next = new Set(prev);
      if (next.has(profileId)) next.delete(profileId);
      else next.add(profileId);
      return next;
    });
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    await onSave({
      id: currentExtension.id,
      name: trimmed,
      defaultEnabled,
      profileIds: Array.from(profileIds),
    });
  }

  return (
    <Modal
      open={open}
      title={t("extensions.edit.title")}
      onClose={onClose}
      size="lg"
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" type="submit" form="extension-edit-form" disabled={busy}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <form
        id="extension-edit-form"
        className="extensions-edit-form"
        onSubmit={(event) => void submit(event)}
      >
        <TextField
          label={t("extensions.field.name")}
          value={name}
          onChange={(event) => setName(event.currentTarget.value)}
        />
        <label className="extensions-checkbox-row">
          <Checkbox
            checked={defaultEnabled}
            onChange={(event) => setDefaultEnabled(event.currentTarget.checked)}
          />
          <span>
            <strong>{t("extensions.field.defaultEnabled")}</strong>
            <small>{t("extensions.field.defaultEnabled.hint")}</small>
          </span>
        </label>
        <div className="extensions-assignment">
          <div className="extensions-assignment__head">
            <strong>{t("extensions.assign.title")}</strong>
            <span>{t("extensions.assigned.count", { count: profileIds.size })}</span>
          </div>
          {profiles.length === 0 ? (
            <div className="extensions-assignment__empty">{t("extensions.assign.empty")}</div>
          ) : null}
          {profiles.map((profile) => (
            <label key={profile.id} className="extensions-profile-option">
              <Checkbox
                checked={profileIds.has(profile.id)}
                onChange={() => toggleProfile(profile.id)}
              />
              <span>
                <strong>{profile.name}</strong>
                <small>{profile.platform || profile.os}</small>
              </span>
            </label>
          ))}
        </div>
      </form>
    </Modal>
  );
}

function parseChromeStoreId(value: string): StorePreview | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const idPattern = /^[a-p]{32}$/;
  if (idPattern.test(trimmed)) return { id: trimmed, name: null };
  const withoutQuery = trimmed.split(/[?#]/, 1)[0] ?? "";
  const parts = withoutQuery.split("/").filter(Boolean);
  const id = parts.find((part) => idPattern.test(part));
  if (!id) return null;
  const idIndex = parts.indexOf(id);
  const detailIndex = parts.findIndex((part) => part.toLowerCase() === "detail");
  const slug = detailIndex >= 0 && idIndex > detailIndex + 1 ? (parts[idIndex - 1] ?? "") : "";
  return {
    id,
    name: formatChromeStoreSlug(slug),
  };
}

function formatChromeStoreSlug(slug: string): string | null {
  const normalized = decodeURIComponent(slug).replace(/[-_]+/g, " ").replace(/\s+/g, " ").trim();
  if (!normalized) return null;
  return normalized.replace(/\b[a-z]/g, (char) => char.toUpperCase());
}
