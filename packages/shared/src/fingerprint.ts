/**
 * Per-profile fingerprint configuration. Mirrors `veil_core::Fingerprint` (camelCase serde).
 * Veil never implements fingerprint logic — these values are mapped to engine launch flags by
 * the backend. Empty / zero fields mean "use the engine default" and emit no flag.
 */

import type { FlagDialect } from "./runtime";

export type WebrtcPolicy = "default" | "proxyOnly" | "disabled";
export type NoiseMode = "off" | "low" | "high";

export interface Fingerprint {
  /** Engine seed: 0 = random each launch; non-zero emits `--fingerprint=<seed>`. */
  seed: number;
  userAgent: string;
  /**
   * Window size, not `screen.*` — emitted as `--window-size` under both dialects. The key name
   * predates `960` and is kept so a stored profile's window cannot be re-read as a panel; the label
   * the user sees says "window" as of T06.
   */
  screenWidth: number;
  screenHeight: number;
  /**
   * The display panel the profile declares, in DIPs. Reaches `screen.width` / `screen.height` via
   * `--fingerprint-screen-{width,height}`, which only a `veil-patched` engine reads. `screen.avail*`,
   * `colorDepth`, `orientation` and `isExtended` are derived from this pair inside the engine and
   * have no field of their own.
   */
  panelWidth: number;
  panelHeight: number;
  /** `window.devicePixelRatio` and `(resolution:)`, via `--fingerprint-device-scale-factor`. */
  devicePixelRatio: number;
  timezone: string;
  locale: string;
  languages: string[];
  webglVendor: string;
  webglRenderer: string;
  canvasNoise: NoiseMode;
  audioNoise: NoiseMode;
  fonts: string[];
  hardwareConcurrency: number;
  /**
   * `navigator.deviceMemory` in GB, via `--fingerprint-device-memory` (`905`). `0` = leave the
   * host's, which is what the engine falls back to.
   *
   * Only {@link DEVICE_MEMORY_GB}'s three values are storable: Chromium can report five, and the two
   * smallest would contradict the JS heap limit the same page reads.
   */
  deviceMemory: number;
  /**
   * `navigator.maxTouchPoints`, and with it `(pointer:)`, `(any-pointer:)`, `(hover:)` and
   * `(any-hover:)` — `961` answers all five from this one number.
   *
   * `null` is "leave the host's". It is not `0`, because a Mac reports **0** touch points
   * structurally and a profile has to be able to say so; this is the one field where the unset
   * sentinel and a real value would collide.
   */
  maxTouchPoints: number | null;
  /**
   * The persona's declared position, as `"lat,lng"` — the exact value `970` reads off
   * `--fingerprint-location` and `navigator.geolocation` then reports. The one geography field a
   * profile carries: Veil is offline and holds no geo-IP table, so the coordinates are the user's
   * to point at, and `validate` refuses coordinates without the timezone that locates them in time.
   */
  location: string;
  webrtcPolicy: WebrtcPolicy;
}

/**
 * The RAM sizes a Veil profile may declare — a **subset** of what Chromium can report.
 *
 * Mirrors `veil_core::DEVICE_MEMORY_GB`. Chromium rounds the host's physical memory to the nearer
 * power of two and clamps into `[2, 32]` on desktop, so it can report five values: 2, 4, 8, 16, 32.
 * Veil offers the top three, because V8 sizes its heap from physical memory and
 * `performance.memory.jsHeapSizeLimit` saturates at 8 GB — a claim of 2 or 4 would contradict the heap
 * limit the same page reads, and closing that would mean reporting a limit the runtime does not
 * honour.
 */
export const DEVICE_MEMORY_GB = [8, 16, 32] as const;

/** A fresh, all-default fingerprint: launches with the engine's stock identity. */
export function emptyFingerprint(): Fingerprint {
  return {
    seed: 0,
    userAgent: "",
    screenWidth: 0,
    screenHeight: 0,
    panelWidth: 0,
    panelHeight: 0,
    devicePixelRatio: 0,
    timezone: "",
    locale: "",
    languages: [],
    webglVendor: "",
    webglRenderer: "",
    canvasNoise: "off",
    audioNoise: "off",
    fonts: [],
    hardwareConcurrency: 0,
    deviceMemory: 0,
    maxTouchPoints: null,
    location: "",
    webrtcPolicy: "default",
  };
}

/** Fingerprint fields whose treatment depends on the installed engine's flag dialect. */
export type DialectGatedFingerprintField =
  | "seed"
  | "timezone"
  | "panelWidth"
  | "panelHeight"
  | "devicePixelRatio"
  | "webglVendor"
  | "webglRenderer"
  | "canvasNoise"
  | "audioNoise"
  | "fonts"
  | "hardwareConcurrency"
  | "deviceMemory"
  | "maxTouchPoints"
  | "location";

/**
 * What an engine does with a stored field. These three states
 * are genuinely different and the UI must not collapse them, because "unsupported" is the wrong
 * word for a surface that *is* spoofed — just not from the value the user typed.
 *
 * - `settable` — the value reaches the engine and is used as given.
 * - `spoofedNotConfigurable` — the surface is spoofed, seed-derived, and this value is not what
 *   drives it. Protection is real; control over it is not.
 * - `unsupported` — no switch exists, so the value reaches nothing and the surface is whatever the
 *   host reports.
 */
export type FingerprintFieldSupport = "settable" | "spoofedNotConfigurable" | "unsupported";

/**
 * Fields the given dialect cannot honour at all. Mirrors `veil_core::to_engine_args`: a field
 * listed here emits no switch, so offering it in the UI would tell the user a profile is spoofed
 * when the engine ignored the setting entirely.
 *
 * `timezone` is listed under `stock-chromium` because stock Chromium has no timezone switch and
 * **ignores the `TZ` environment variable on Windows** — measured against Chromium 151.
 *
 * Under `veil-patched` the set is three fields, measured out of the patch set (5.3) rather than
 * inferred: Tier 1 picks the WebGL unmasked vendor/renderer pair from a built-in table indexed by
 * `fingerprint % GetGpuCount()`, and `006` hides a seed-derived subset of the host's font families.
 * Neither takes a value, so `--webgl-vendor`, `--webgl-renderer` and `--fonts` exist nowhere.
 *
 * `panelWidth`, `panelHeight` and `devicePixelRatio` **left** this set for `veil-patched` in Stage 6's
 * T06, in the order §1 requires: `960` exists and its surfaces were measured to move before
 * `to_engine_args` gained the switches, and this mirror moved in the same commit. They stay
 * unsupported under `stock-chromium`, which has no switch for any of the three — `--window-size`
 * moves the window and never `screen.*`.
 *
 * `maxTouchPoints` joined the `veil-patched` settable set the same way in phase 4 (`961`), and
 * `deviceMemory` moved out of the *third* category rather than out of this one (`905`). `location`
 * never joined this set for `veil-patched`: it arrived as settable in Stage 6's T22 in §1's order —
 * `970` exists, builds, and its surface was measured to move before this mirror gained the field.
 */
export function unsupportedFingerprintFields(
  dialect: FlagDialect,
): ReadonlySet<DialectGatedFingerprintField> {
  if (dialect === "veil-patched") return new Set(["webglVendor", "webglRenderer", "fonts"]);
  return new Set([
    "seed",
    "timezone",
    "panelWidth",
    "panelHeight",
    "devicePixelRatio",
    "webglVendor",
    "webglRenderer",
    "canvasNoise",
    "audioNoise",
    "fonts",
    "hardwareConcurrency",
    "deviceMemory",
    "maxTouchPoints",
    "location",
  ]);
}

/**
 * Fields whose surface the given dialect spoofs while ignoring the stored value — 5.4's third
 * category, and the reason it needs to exist.
 *
 * Under `veil-patched`:
 *
 * - `canvasNoise` / `audioNoise` — the amount is derived from `--fingerprint=<seed>`. There is no
 *   `--canvas-noise` / `--audio-noise`, so `low` and `high` are the same instruction to the engine.
 *   Only `off` is expressible, as a `--disable-spoofing=` key, which is why these stay editable:
 *   the on/off half is honoured and the level half is not.
 * - `deviceMemory` was the third member until Stage 6 T15. `005` returned 8 from
 *   `NavigatorDeviceMemory::deviceMemory()` unconditionally — no switch, no seed (measured, 5.1e) —
 *   and `905` replaced that body with a `--fingerprint-device-memory` read, so the field is now
 *   *settable* and this set is down to the noise pair. It moved in §1's order: patch, measurement,
 *   then the three mirrors together.
 *
 * Empty under `stock-chromium`: nothing is spoofed there, so every gated field is simply
 * unsupported.
 */
export function spoofedNotConfigurableFingerprintFields(
  dialect: FlagDialect,
): ReadonlySet<DialectGatedFingerprintField> {
  if (dialect === "veil-patched") return new Set(["canvasNoise", "audioNoise"]);
  return new Set();
}

/**
 * Everything the View needs to render one field honestly, so it needs no rule of its own.
 *
 * `noteKey` is an `@veil/i18n` message key, resolved by the caller — the same arrangement as
 * `AppErrorCode` → `errors.<code>`. The third category gets a **per-field** key rather than one
 * generic sentence: it held three fields that failed differently until T15 moved `deviceMemory` out,
 * and the two that remain are still worth naming individually rather than covering with a vague
 * sentence.
 */
export interface FingerprintFieldState {
  support: FingerprintFieldSupport;
  /**
   * `false` when no part of the stored value reaches the engine, which is when the control is
   * disabled. Canvas / audio noise stay editable under `veil-patched` even though they are
   * category 3: `off` is expressible as a `--disable-spoofing=` key, so the switch half of the
   * control is real and only the level is not.
   */
  editable: boolean;
  noteKey: string | null;
}

/**
 * The fields `960` derives from `--fingerprint-platform` rather than reading on its own: the panel
 * pair and the scale factor. `screen.avail*`, `colorDepth`, `orientation` and `isExtended` are
 * derived from them inside the engine and have no field here.
 */
const PLATFORM_DERIVED_FIELDS: ReadonlySet<DialectGatedFingerprintField> = new Set([
  "panelWidth",
  "panelHeight",
  "devicePixelRatio",
]);

/** Which of the three states the given dialect puts a field in, and how to present it. */
export function fingerprintFieldState(
  dialect: FlagDialect,
  field: DialectGatedFingerprintField,
  os?: string,
): FingerprintFieldState {
  if (unsupportedFingerprintFields(dialect).has(field)) {
    return { support: "unsupported", editable: false, noteKey: "fingerprint.field.unsupported" };
  }
  // The platform gate, ahead of the dialect's third category: `960` derives the display block's
  // companions from `--fingerprint-platform`, and an OS that claims no platform gets no such switch,
  // so the stored panel reaches nothing on *any* engine. `veil_core::validate` refuses the pair and
  // `to_engine_args` emits none of it, which is what makes "unsupported" here an accurate word
  // rather than the understatement 5.4 warns about for a spoofed surface.
  if (os !== undefined && osClaimsNoPlatform(os) && PLATFORM_DERIVED_FIELDS.has(field)) {
    return {
      support: "unsupported",
      editable: false,
      noteKey: "fingerprint.field.displayNeedsPlatform",
    };
  }
  if (spoofedNotConfigurableFingerprintFields(dialect).has(field)) {
    return {
      support: "spoofedNotConfigurable",
      editable: field === "canvasNoise" || field === "audioNoise",
      noteKey: `fingerprint.field.spoofedNotConfigurable.${field}`,
    };
  }
  return { support: "settable", editable: true, noteKey: null };
}

/**
 * Whether the seed, rather than the stored `userAgent`, owns this profile's identity.
 *
 * Mirrors the rule in `veil_core::validate`: a non-zero seed and a literal `userAgent` are two
 * competing sources of UA truth, so storing both is rejected. This is *not* a dialect question —
 * `validate` takes no dialect, so that a stored profile stays valid across an engine swap — which is
 * why it is a separate helper rather than a fourth `FingerprintFieldState`.
 *
 * Measured, and it is worse than redundancy: a literal `--user-agent=` reaches only the UA string and
 * makes Chromium stop populating the UA-CH high-entropy hints, so `platformVersion` and `architecture`
 * come back as **empty strings**, which no real browser returns. On a patched engine the coherent way
 * to present another platform is the profile's `os`, which becomes `--fingerprint-platform=`.
 */
export function seedOwnsUserAgent(fingerprint: Fingerprint): boolean {
  return fingerprint.seed !== 0;
}

/**
 * Whether the declared display panel and scale factor can reach the engine at all.
 *
 * Mirrors the gate in `veil_core::to_engine_args` and the matching rule in `veil_core::validate`:
 * `960` derives `screen.avail*`, `colorDepth` and `orientation` from `--fingerprint-platform`, and
 * that switch is emitted only alongside a seed. A panel with no seed would be given a Windows
 * taskbar inset on a macOS profile — a work area agreeing with neither side — so the profile is
 * rejected at save time instead.
 *
 * Like {@link seedOwnsUserAgent} this is a cross-field rule rather than a dialect one, which is why
 * it is a helper and not a fourth `FingerprintFieldState`: the answer is the same on both dialects.
 */
export function displayPanelNeedsSeed(fingerprint: Fingerprint): boolean {
  return fingerprint.seed === 0;
}

/**
 * Whether this profile's OS forbids declaring any touch points at all.
 *
 * Mirrors the rule in `veil_core::validate`, and on this engine it is structural rather than a
 * statement about Apple's product line: macOS builds `ui/base/pointer/pointer_device_default.cc`,
 * whose `MaxTouchPoints()` returns a constant `0`, so no Chromium on a Mac can report a touchscreen
 * even if Apple ships one. A macOS profile may store `0` or leave the field unset; anything else is
 * refused at save time, which is what keeps `macos-reports-no-touch-points` a rule about captures.
 *
 * Third of the cross-field helpers, alongside {@link seedOwnsUserAgent} and
 * {@link displayPanelNeedsSeed}, and a helper for the same reason: the answer depends on the
 * profile's OS rather than on the installed engine's dialect.
 */
export function osForbidsTouchPoints(os: string): boolean {
  return os === "macos";
}

/**
 * Whether this profile's OS makes no platform claim at all — `ios`, the one value nothing
 * anywhere implements (6.24) and the reason the display block stays unsupported on a profile
 * that carries one. `android` left this helper when the port landed (6.26): the engine now
 * answers the claim, the picker offers the value again, and the display block is required on it
 * rather than forbidden.
 *
 * Mirrors `veil_core::ProfileOs::has_fingerprint_platform`. `to_engine_args` emits
 * `--fingerprint-platform` for the three desktop values and android, and `960` derives the work
 * area, the colour depth and the orientation from exactly that switch — so an iOS profile can
 * declare no coherent display block. `veil_core::validate` refuses the pair for the same reason.
 *
 * Fourth of the cross-field helpers, alongside {@link seedOwnsUserAgent},
 * {@link displayPanelNeedsSeed} and {@link osForbidsTouchPoints}, and a helper for the same reason:
 * the answer depends on the profile's OS rather than on the installed engine's dialect.
 */
export function osClaimsNoPlatform(os: string): boolean {
  return os === "ios";
}

/**
 * Whether an android profile must declare a non-zero touch count — the phone-side twin of
 * {@link osForbidsTouchPoints}: macOS structurally has no touchscreen, a structurally has
 * nothing but one (6.26). Mirrors `veil_core::validate`'s android rules, which also require the
 * portrait panel and scale factor; the touch count is the one with its own field, so it gets
 * its own helper the UI can read.
 */
export function androidRequiresTouchPoints(os: string): boolean {
  return os === "android";
}
