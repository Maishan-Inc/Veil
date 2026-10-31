import { type ReactNode } from "react";
import { useT } from "@veil/i18n";
import {
  DEVICE_MEMORY_GB,
  displayPanelNeedsSeed,
  fingerprintFieldState,
  osForbidsTouchPoints,
  seedOwnsUserAgent,
  type DialectGatedFingerprintField,
  type Fingerprint,
  type FlagDialect,
  type NoiseMode,
  type ProfileOS,
  type WebrtcPolicy,
} from "@veil/shared";
import { Banner, Button, Select, TextField } from "@veil/ui";
import { randomizeFingerprint } from "../../api/client";

export interface FingerprintSectionProps {
  os: ProfileOS;
  value: Fingerprint;
  onChange: (next: Fingerprint) => void;
  /** Switch vocabulary of the installed engine; decides which fields can be honoured. */
  flagDialect: FlagDialect;
  /**
   * Whether `flagDialect` was read off the installed engine rather than defaulted. `false` means
   * nothing is recorded, which behaves like `stock-chromium` but must not be described as it.
   */
  dialectRecorded: boolean;
  /** Engine name from the manifest, named in the degraded-capability banner. */
  engineName: string;
  onOptimizeByProxy?: (() => void) | undefined;
  optimizingByProxy?: boolean | undefined;
}

const NOISE: NoiseMode[] = ["off", "low", "high"];
const WEBRTC: WebrtcPolicy[] = ["default", "proxyOnly", "disabled"];

/**
 * Fingerprint editor embedded in the profile dialog. The View stays dumb: randomization and
 * validation are backend concerns, so "Randomize" just asks the service for a coherent preset.
 *
 * Whether a field is editable and what note sits under it comes from
 * `fingerprintFieldState(dialect, field)` — three states, not two. That is a correctness
 * requirement, not polish: a stock engine ignores unknown switches silently, so an editable
 * canvas-noise field would claim protection that does not exist, and on a patched engine calling a
 * *spoofed* surface "unsupported" understates it in the other direction.
 *
 * The banner has three states for a different reason. Two are about capability and one is about
 * *knowledge*: an engine installed before Veil recorded a dialect behaves exactly like stock, but
 * saying "this engine provides no fingerprint protection" about it would assert a measurement nobody
 * took. So `dialectRecorded === false` gets its own wording and a way out (reinstall), while the
 * field gating stays identical to stock — degrading behaviour on unknown capability is the only safe
 * direction.
 */
export function FingerprintSection({
  os,
  value,
  onChange,
  flagDialect,
  dialectRecorded,
  engineName,
  onOptimizeByProxy,
  optimizingByProxy = false,
}: FingerprintSectionProps): ReactNode {
  const t = useT();
  const degraded = flagDialect === "stock-chromium";
  const banner = degraded ? (dialectRecorded ? "stock" : "unknown") : null;
  const set = <K extends keyof Fingerprint>(key: K, v: Fingerprint[K]): void =>
    onChange({ ...value, [key]: v });
  const field = (name: DialectGatedFingerprintField) => {
    const { editable, noteKey } = fingerprintFieldState(flagDialect, name, os);
    return { disabled: !editable, hint: noteKey ? t(noteKey) : undefined };
  };
  // Not a dialect question but a cross-field one, so it does not go through `field()`: a seed and a
  // literal user agent are two competing sources of UA truth and `veil_core::validate` rejects the
  // pair. Disabling the control is what stops the dialog building a profile that cannot be saved.
  const uaOwnedBySeed = seedOwnsUserAgent(value);
  // The same shape for the display block, and for the same reason: `960` derives the work area and
  // the orientation from `--fingerprint-platform`, which is emitted only alongside a seed, so a panel
  // without one is a profile `validate` refuses. The gate is combined with the dialect's, because a
  // field can be unreachable for either reason and the note has to say which. It also carries the
  // platform gate: a mobile profile claims no platform at all, so its display block is unsupported
  // before the seed question is even asked — `fingerprintFieldState` answers that, and the note it
  // returns is the reason the user sees.
  const panelNeedsSeed = displayPanelNeedsSeed(value);
  const displayField = (name: DialectGatedFingerprintField) => {
    const { editable, noteKey } = fingerprintFieldState(flagDialect, name, os);
    if (!editable) {
      return { disabled: true, hint: noteKey ? t(noteKey) : undefined };
    }
    return panelNeedsSeed
      ? { disabled: true, hint: t("fingerprint.field.displayNeedsSeed") }
      : { disabled: false, hint: undefined };
  };
  // Third of the same shape, and the cross-field rule is the profile's OS rather than its seed: macOS
  // builds a Chromium whose `MaxTouchPoints()` is a constant 0, so a Mac profile cannot declare a
  // touchscreen and `veil_core::validate` refuses one. Disabling the control is what stops the dialog
  // building a profile that cannot be saved.
  const touchField = () => {
    const { editable, noteKey } = fingerprintFieldState(flagDialect, "maxTouchPoints", os);
    if (!editable) {
      return { disabled: true, hint: noteKey ? t(noteKey) : undefined };
    }
    return osForbidsTouchPoints(os)
      ? { disabled: true, hint: t("fingerprint.field.maxTouchPoints.macos") }
      : { disabled: false, hint: undefined };
  };

  async function randomize(): Promise<void> {
    try {
      onChange(await randomizeFingerprint(os, Math.floor(Math.random() * 1e9)));
    } catch {
      /* generation never fails; ignore */
    }
  }

  return (
    <fieldset className="veil-fp">
      <div className="veil-fp__head">
        <span className="veil-fp__title">{t("fingerprint.section")}</span>
        <Button variant="secondary" type="button" onClick={() => void randomize()}>
          {t("fingerprint.randomize")}
        </Button>
        {onOptimizeByProxy ? (
          <Button
            variant="secondary"
            type="button"
            onClick={onOptimizeByProxy}
            disabled={optimizingByProxy}
          >
            {t(
              optimizingByProxy
                ? "fingerprint.optimizeByProxy.running"
                : "fingerprint.optimizeByProxy",
            )}
          </Button>
        ) : null}
      </div>
      <p className="veil-fp__hint">{t("fingerprint.subtitle")}</p>

      {banner ? (
        <Banner tone="warning" title={t(`fingerprint.dialect.${banner}.title`)}>
          {t(`fingerprint.dialect.${banner}.body`, { engine: engineName })}
        </Banner>
      ) : null}

      <TextField
        label={t("fingerprint.field.seed")}
        type="number"
        value={value.seed || ""}
        placeholder={t("fingerprint.field.seed.placeholder")}
        {...field("seed")}
        onChange={(e) => set("seed", Number(e.currentTarget.value) || 0)}
      />
      <TextField
        label={t("fingerprint.field.userAgent")}
        value={value.userAgent}
        disabled={uaOwnedBySeed}
        hint={uaOwnedBySeed ? t("fingerprint.field.seedOwnsUserAgent") : undefined}
        onChange={(e) => set("userAgent", e.currentTarget.value)}
      />
      <div className="veil-fp__row">
        <TextField
          label={t("fingerprint.field.window")}
          value={value.screenWidth ? `${value.screenWidth}x${value.screenHeight}` : ""}
          placeholder="1920x1080"
          hint={t("fingerprint.field.window.hint")}
          onChange={(e) => {
            const [w, h] = e.currentTarget.value.split("x").map((n) => Number(n) || 0);
            onChange({ ...value, screenWidth: w ?? 0, screenHeight: h ?? 0 });
          }}
        />
        <TextField
          label={t("fingerprint.field.timezone")}
          value={value.timezone}
          {...field("timezone")}
          onChange={(e) => set("timezone", e.currentTarget.value)}
        />
        <TextField
          label={t("fingerprint.field.location")}
          value={value.location}
          placeholder={t("fingerprint.field.location.placeholder")}
          {...field("location")}
          onChange={(e) => set("location", e.currentTarget.value)}
        />
      </div>
      <div className="veil-fp__row">
        <TextField
          label={t("fingerprint.field.panel")}
          value={value.panelWidth ? `${value.panelWidth}x${value.panelHeight}` : ""}
          placeholder="1470x956"
          {...displayField("panelWidth")}
          onChange={(e) => {
            const [w, h] = e.currentTarget.value.split("x").map((n) => Number(n) || 0);
            onChange({ ...value, panelWidth: w ?? 0, panelHeight: h ?? 0 });
          }}
        />
        <TextField
          label={t("fingerprint.field.devicePixelRatio")}
          type="number"
          step="0.25"
          value={value.devicePixelRatio || ""}
          placeholder="2"
          {...displayField("devicePixelRatio")}
          onChange={(e) => set("devicePixelRatio", Number(e.currentTarget.value) || 0)}
        />
      </div>
      <div className="veil-fp__row">
        <TextField
          label={t("fingerprint.field.locale")}
          value={value.locale}
          onChange={(e) => set("locale", e.currentTarget.value)}
        />
        <TextField
          label={t("fingerprint.field.languages")}
          value={value.languages.join(",")}
          placeholder={t("fingerprint.field.languages.placeholder")}
          onChange={(e) => set("languages", splitList(e.currentTarget.value))}
        />
      </div>
      <div className="veil-fp__row">
        <TextField
          label={t("fingerprint.field.webglVendor")}
          value={value.webglVendor}
          {...field("webglVendor")}
          onChange={(e) => set("webglVendor", e.currentTarget.value)}
        />
        <TextField
          label={t("fingerprint.field.webglRenderer")}
          value={value.webglRenderer}
          {...field("webglRenderer")}
          onChange={(e) => set("webglRenderer", e.currentTarget.value)}
        />
      </div>
      <div className="veil-fp__row">
        <Select
          label={t("fingerprint.field.canvasNoise")}
          value={value.canvasNoise}
          {...field("canvasNoise")}
          onChange={(e) => set("canvasNoise", e.currentTarget.value as NoiseMode)}
          options={NOISE.map((n) => ({ value: n, label: t(`fingerprint.noise.${n}`) }))}
        />
        <Select
          label={t("fingerprint.field.audioNoise")}
          value={value.audioNoise}
          {...field("audioNoise")}
          onChange={(e) => set("audioNoise", e.currentTarget.value as NoiseMode)}
          options={NOISE.map((n) => ({ value: n, label: t(`fingerprint.noise.${n}`) }))}
        />
      </div>
      <TextField
        label={t("fingerprint.field.fonts")}
        value={value.fonts.join(",")}
        placeholder={t("fingerprint.field.fonts.placeholder")}
        {...field("fonts")}
        onChange={(e) => set("fonts", splitList(e.currentTarget.value))}
      />
      <div className="veil-fp__row">
        <TextField
          label={t("fingerprint.field.hardwareConcurrency")}
          type="number"
          value={value.hardwareConcurrency || ""}
          {...field("hardwareConcurrency")}
          onChange={(e) => set("hardwareConcurrency", Number(e.currentTarget.value) || 0)}
        />
        <Select
          label={t("fingerprint.field.deviceMemory")}
          value={value.deviceMemory ? String(value.deviceMemory) : ""}
          placeholder={t("fingerprint.field.deviceMemory.host")}
          {...field("deviceMemory")}
          onChange={(e) => set("deviceMemory", Number(e.currentTarget.value) || 0)}
          options={DEVICE_MEMORY_GB.map((gb) => ({ value: String(gb), label: `${gb} GB` }))}
        />
      </div>
      <TextField
        label={t("fingerprint.field.maxTouchPoints")}
        type="number"
        value={value.maxTouchPoints === null ? "" : String(value.maxTouchPoints)}
        placeholder={t("fingerprint.field.maxTouchPoints.host")}
        {...touchField()}
        onChange={(e) =>
          set(
            "maxTouchPoints",
            e.currentTarget.value === "" ? null : Number(e.currentTarget.value) || 0,
          )
        }
      />
      <Select
        label={t("fingerprint.field.webrtc")}
        value={value.webrtcPolicy}
        onChange={(e) => set("webrtcPolicy", e.currentTarget.value as WebrtcPolicy)}
        options={WEBRTC.map((p) => ({ value: p, label: t(`fingerprint.webrtc.${p}`) }))}
      />
    </fieldset>
  );
}

function splitList(raw: string): string[] {
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}
