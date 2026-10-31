import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import {
  emptyFingerprint,
  emptyInstalledRuntimes,
  isSelectableProfileOs,
  SELECTABLE_PROFILE_OS,
  type CreateProfileInput,
  type Fingerprint,
  type Group,
  type InstalledRuntimes,
  type Profile,
  type ProfileOS,
  type Proxy,
  type StartupPage,
} from "@veil/shared";
import { Banner, Button, Modal, Select, TextField } from "@veil/ui";
import {
  AppError,
  createProfile,
  listGroups,
  listInstalledRuntimes,
  listProxies,
  optimizeFingerprintForProxy,
  updateProfile,
} from "../../api/client";
import { formatProxyOption } from "../proxy/format";
import { FingerprintSection } from "./FingerprintSection";
import { osLabel } from "./format";

export interface ProfileFormDialogProps {
  open: boolean;
  /** Editing mode when provided; create mode when undefined. */
  profile?: Profile | undefined;
  onClose: () => void;
  onSaved: (profile: Profile) => void;
}

interface FormState {
  name: string;
  os: ProfileOS;
  platform: string;
  groupId: string;
  proxyId: string;
  browserVersion: string;
  startupPage: StartupPage;
  startupUrl: string;
  notes: string;
  fingerprint: Fingerprint;
}

function emptyForm(): FormState {
  return {
    name: "",
    os: "windows",
    platform: "",
    groupId: "",
    proxyId: "",
    browserVersion: "",
    startupPage: "veil",
    startupUrl: "",
    notes: "",
    fingerprint: emptyFingerprint(),
  };
}

function fromProfile(profile: Profile): FormState {
  return {
    name: profile.name,
    os: profile.os,
    platform: profile.platform,
    groupId: profile.groupId ?? "",
    proxyId: profile.proxyId ?? "",
    browserVersion: profile.browserVersion,
    startupPage: profile.startupPage,
    startupUrl: profile.startupUrl,
    notes: profile.notes,
    fingerprint: profile.fingerprint,
  };
}

/**
 * Create / edit a single profile. Same dialog shape regardless of mode; the existence of
 * `profile` decides which API path runs. Errors come back as `AppError` from the API
 * client and are surfaced as a localized Banner inside the form.
 */
export function ProfileFormDialog({
  open,
  profile,
  onClose,
  onSaved,
}: ProfileFormDialogProps): ReactNode {
  const t = useT();
  const editing = Boolean(profile);
  const [form, setForm] = useState<FormState>(() => (profile ? fromProfile(profile) : emptyForm()));
  const [groups, setGroups] = useState<Group[]>([]);
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [runtimes, setRuntimes] = useState<InstalledRuntimes>(emptyInstalledRuntimes);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [noticeKey, setNoticeKey] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [optimizingByProxy, setOptimizingByProxy] = useState(false);

  // Reset form whenever the dialog re-opens with a new target.
  useEffect(() => {
    if (!open) return;
    setForm(profile ? fromProfile(profile) : emptyForm());
    setErrorCode(null);
    setNoticeKey(null);
    setSubmitting(false);
    setOptimizingByProxy(false);
  }, [open, profile]);

  // Lazy load supporting lists once when the dialog opens.
  useEffect(() => {
    if (!open) return;
    listGroups()
      .then(setGroups)
      .catch(() => setGroups([]));
    listProxies()
      .then(setProxies)
      .catch(() => setProxies([]));
    listInstalledRuntimes()
      .then((next) => {
        setRuntimes(next);
        if (editing) return;
        const defaultVersion = next.currentVersion ?? next.installed[0]?.version ?? "";
        if (!defaultVersion) return;
        setForm((prev) =>
          prev.browserVersion ? prev : { ...prev, browserVersion: defaultVersion },
        );
      })
      .catch(() => setRuntimes(emptyInstalledRuntimes()));
  }, [editing, open]);

  function patch<K extends keyof FormState>(key: K, value: FormState[K]): void {
    setForm((prev) => ({ ...prev, [key]: value }));
    if (key === "proxyId") setNoticeKey(null);
  }

  async function handleOptimizeByProxy(): Promise<void> {
    if (optimizingByProxy) return;
    if (!form.proxyId) {
      setNoticeKey("fingerprint.optimizeByProxy.noProxy");
      return;
    }
    setOptimizingByProxy(true);
    setErrorCode(null);
    setNoticeKey(null);
    try {
      const result = await optimizeFingerprintForProxy({
        proxyId: form.proxyId,
        os: form.os,
        seed: Math.floor(Math.random() * 1e9),
      });
      setForm((prev) => ({ ...prev, fingerprint: result.fingerprint }));
      setProxies((prev) =>
        [...prev.filter((proxy) => proxy.id !== result.proxy.id), result.proxy].sort((a, b) =>
          a.name.localeCompare(b.name),
        ),
      );
      setNoticeKey("fingerprint.optimizeByProxy.success");
    } catch (err) {
      setErrorCode(err instanceof AppError ? err.code : "app.unknown");
    } finally {
      setOptimizingByProxy(false);
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (submitting) return;

    setSubmitting(true);
    setErrorCode(null);

    const input: CreateProfileInput = {
      name: form.name.trim(),
      os: form.os,
      platform: form.platform.trim(),
      browserVersion: form.browserVersion,
      groupId: form.groupId || null,
      proxyId: form.proxyId || null,
      tagIds: profile?.tagIds ?? [],
      notes: form.notes,
      startupPage: form.startupPage,
      startupUrl: form.startupUrl,
      fingerprint: form.fingerprint,
    };

    try {
      const saved =
        editing && profile ? await updateProfile(profile.id, input) : await createProfile(input);
      onSaved(saved);
    } catch (err) {
      setErrorCode(err instanceof AppError ? err.code : "app.unknown");
      setSubmitting(false);
    }
  }

  const selectedProxy = proxies.find((proxy) => proxy.id === form.proxyId);
  const selectedProxyHasUnsupportedCredentials = Boolean(
    selectedProxy && (selectedProxy.username || selectedProxy.hasPassword),
  );
  const runtimeOptions = [
    ...runtimes.installed.map((runtime) => ({
      value: runtime.version,
      label:
        runtime.version === runtimes.currentVersion
          ? `${runtime.version} - ${t("profiles.runtime.current")}`
          : runtime.version,
    })),
  ];
  if (
    form.browserVersion &&
    !runtimeOptions.some((option) => option.value === form.browserVersion)
  ) {
    runtimeOptions.unshift({
      value: form.browserVersion,
      label: `${form.browserVersion} - ${t("profiles.runtime.unavailable")}`,
    });
  }

  return (
    <Modal
      open={open}
      title={t(editing ? "profiles.dialog.edit.title" : "profiles.dialog.create.title")}
      onClose={submitting ? () => undefined : onClose}
      size="lg"
      dismissOnBackdrop={!submitting}
      dismissOnEscape={!submitting}
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={submitting}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" type="submit" form="veil-profile-form" disabled={submitting}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <form id="veil-profile-form" className="veil-form" onSubmit={(e) => void handleSubmit(e)}>
        {errorCode ? (
          <Banner tone="error" title={t(`errors.${errorCode}`)}>
            {null}
          </Banner>
        ) : null}
        {noticeKey ? (
          <Banner tone={noticeKey.endsWith(".success") ? "success" : "warning"}>
            {t(noticeKey)}
          </Banner>
        ) : null}

        <TextField
          label={t("profiles.field.name")}
          value={form.name}
          onChange={(e) => patch("name", e.currentTarget.value)}
          placeholder={t("profiles.field.name.placeholder")}
          autoFocus
          required
        />

        <Select
          label={t("profiles.field.os")}
          value={form.os}
          onChange={(e) => patch("os", e.currentTarget.value as ProfileOS)}
          // `android` and `ios` left the picker in T24: no patch in the set implements a mobile
          // platform, so the only thing selecting one could do was present the host's own platform.
          // A profile stored before the change still carries its value, and it is shown here —
          // disabled, named — so the control never goes blank and the value can never be chosen
          // again.
          options={[
            ...(isSelectableProfileOs(form.os)
              ? []
              : [
                  {
                    value: form.os,
                    label: t("profiles.field.os.storedValue", { os: t(osLabel(form.os)) }),
                    disabled: true,
                  },
                ]),
            ...SELECTABLE_PROFILE_OS.map((value) => ({ value, label: t(osLabel(value)) })),
          ]}
        />

        <TextField
          label={t("profiles.field.platform")}
          value={form.platform}
          onChange={(e) => patch("platform", e.currentTarget.value)}
          placeholder={t("profiles.field.platform.placeholder")}
        />

        <Select
          label={t("profiles.field.group")}
          value={form.groupId}
          onChange={(e) => patch("groupId", e.currentTarget.value)}
          options={[
            { value: "", label: t("profiles.noGroup") },
            ...groups.map((g) => ({ value: g.id, label: g.name })),
          ]}
        />

        <Select
          label={t("profiles.field.proxy")}
          value={form.proxyId}
          onChange={(e) => patch("proxyId", e.currentTarget.value)}
          options={[
            { value: "", label: t("profiles.noProxy") },
            ...proxies.map((p) => ({ value: p.id, label: formatProxyOption(p) })),
          ]}
        />

        {selectedProxyHasUnsupportedCredentials ? (
          <Banner tone="warning" title={t("proxy.warning.credentialsLaunchUnsupported.title")}>
            {t("proxy.warning.credentialsLaunchUnsupported.body")}
          </Banner>
        ) : null}

        <Select
          label={t("profiles.field.browserVersion")}
          value={form.browserVersion}
          onChange={(e) => patch("browserVersion", e.currentTarget.value)}
          placeholder={t("profiles.runtime.selectPlaceholder")}
          options={runtimeOptions}
        />

        {runtimes.installed.length === 0 ? (
          <Banner tone="warning">{t("profiles.runtime.noneInstalled")}</Banner>
        ) : null}

        <Select
          label={t("profiles.field.startupPage")}
          value={form.startupPage}
          onChange={(e) => patch("startupPage", e.currentTarget.value as StartupPage)}
          options={[
            { value: "veil", label: t("profiles.field.startupPage.veil") },
            { value: "google", label: t("profiles.field.startupPage.google") },
            { value: "bing", label: t("profiles.field.startupPage.bing") },
            { value: "custom", label: t("profiles.field.startupPage.custom") },
          ]}
        />

        {form.startupPage === "custom" ? (
          <TextField
            label={t("profiles.field.startupUrl")}
            value={form.startupUrl}
            onChange={(e) => patch("startupUrl", e.currentTarget.value)}
            placeholder={t("profiles.field.startupUrl.placeholder")}
          />
        ) : null}

        <label className="veil-field">
          <span className="veil-field__label">{t("profiles.field.notes")}</span>
          <textarea
            className="veil-input"
            rows={3}
            value={form.notes}
            onChange={(e) => patch("notes", e.currentTarget.value)}
            placeholder={t("profiles.field.notes.placeholder")}
          />
        </label>

        <FingerprintSection
          os={form.os}
          value={form.fingerprint}
          onChange={(fp) => patch("fingerprint", fp)}
          flagDialect={runtimes.flagDialect}
          dialectRecorded={runtimes.dialectRecorded}
          engineName={runtimes.engineName}
          onOptimizeByProxy={() => void handleOptimizeByProxy()}
          optimizingByProxy={optimizingByProxy}
        />
      </form>
    </Modal>
  );
}
