import { describe, expect, it } from "vitest";
import {
  androidRequiresTouchPoints,
  fingerprintFieldState,
  isLaunchableProfile,
  isSelectableProfileOs,
  seedOwnsUserAgent,
  isRuntimeProgressTerminal,
  profileStatusTone,
  runtimeNeedsInstall,
  SELECTABLE_PROFILE_OS,
  spoofedNotConfigurableFingerprintFields,
  unsupportedFingerprintFields,
  type AppErrorCode,
  type DialectGatedFingerprintField,
  type FlagDialect,
  type Profile,
  type ProfileOS,
  type RuntimeInstallState,
  type RuntimeProgress,
  emptyFingerprint,
  emptyInstalledRuntimes,
} from "./index";

const baseProfile: Profile = {
  id: "profile-1",
  name: "Default",
  avatarColor: "#5640C9",
  groupId: null,
  tagIds: [],
  proxyId: null,
  os: "windows",
  platform: "",
  browserVersion: "120",
  notes: "",
  status: "idle",
  startupPage: "veil",
  startupUrl: "",
  fingerprint: emptyFingerprint(),
  createdAt: "2026-06-27T00:00:00.000Z",
  updatedAt: "2026-06-27T00:00:00.000Z",
  lastLaunchedAt: null,
};

describe("shared contracts", () => {
  it("detects a missing runtime", () => {
    const state: RuntimeInstallState = { kind: "missing" };
    expect(runtimeNeedsInstall(state)).toBe(true);
  });

  it("treats an installed runtime as ready", () => {
    const state: RuntimeInstallState = {
      kind: "installed",
      current: {
        version: "120",
        channel: "free",
        executablePath: "C:/Veil/runtime/120/engine.exe",
        installedAt: "2026-06-27T00:00:00.000Z",
        sha256: "a".repeat(64),
        sizeBytes: 150_000_000,
      },
    };
    expect(runtimeNeedsInstall(state)).toBe(false);
  });

  it("requires a non-empty name before launch", () => {
    expect(isLaunchableProfile({ ...baseProfile, name: "  " })).toBe(false);
    expect(isLaunchableProfile(baseProfile)).toBe(true);
  });

  it("does not relaunch a profile that is already launching", () => {
    expect(isLaunchableProfile({ ...baseProfile, status: "launching" })).toBe(false);
  });

  it("maps status to a semantic tone", () => {
    expect(profileStatusTone("running")).toBe("success");
    expect(profileStatusTone("error")).toBe("error");
    expect(profileStatusTone("idle")).toBe("neutral");
  });

  it("keeps error codes stable as i18n keys", () => {
    const code: AppErrorCode = "runtime.checksumMismatch";
    expect(code).toBe("runtime.checksumMismatch");
  });

  it("detects terminal runtime progress events", () => {
    const completed: RuntimeProgress = { kind: "completed", version: "120" };
    const cancelled: RuntimeProgress = { kind: "cancelled", version: "120" };
    const failed: RuntimeProgress = {
      kind: "failed",
      version: "120",
      code: "runtime.checksumMismatch",
    };
    const downloading: RuntimeProgress = {
      kind: "downloading",
      downloaded: 1,
      total: 10,
      bytesPerSec: 1,
    };
    expect(isRuntimeProgressTerminal(completed)).toBe(true);
    expect(isRuntimeProgressTerminal(cancelled)).toBe(true);
    expect(isRuntimeProgressTerminal(failed)).toBe(true);
    expect(isRuntimeProgressTerminal(downloading)).toBe(false);
  });

  it("assumes no fingerprint capability until an engine reports otherwise", () => {
    // A failed `list_installed_runtimes` must never degrade towards "protected".
    expect(emptyInstalledRuntimes().flagDialect).toBe("stock-chromium");
    // …and must not claim the degradation was measured: nothing has been read yet (5.5c).
    expect(emptyInstalledRuntimes().dialectRecorded).toBe(false);
  });

  it("blocks every patch-set-only field on a stock engine", () => {
    // Mirrors `veil_core::to_engine_args`: these fields emit no switch under `stock-chromium`,
    // and `timezone` is included because Chromium ignores `TZ` on Windows (measured).
    expect([...unsupportedFingerprintFields("stock-chromium")].sort()).toEqual([
      "audioNoise",
      "canvasNoise",
      "deviceMemory",
      "devicePixelRatio",
      "fonts",
      "hardwareConcurrency",
      "location",
      "maxTouchPoints",
      "panelHeight",
      "panelWidth",
      "seed",
      "timezone",
      "webglRenderer",
      "webglVendor",
    ]);
    // Nothing is spoofed on a stock engine, so the third category is empty there.
    expect(spoofedNotConfigurableFingerprintFields("stock-chromium").size).toBe(0);
  });

  it("blocks only the three fields Tier 1 has no switch for on a patched engine", () => {
    // Measured out of the patch set, not inferred: the
    // WebGL unmasked pair comes from a built-in table indexed by the seed and `006` hides a
    // seed-derived subset of the host's families, so none of the three takes a value. Returning
    // an empty set here — which this function used to do — would re-enable six fields the engine
    // cannot honour as typed.
    expect([...unsupportedFingerprintFields("veil-patched")].sort()).toEqual([
      "fonts",
      "webglRenderer",
      "webglVendor",
    ]);
  });

  it("separates spoofed-but-not-configurable from unsupported on a patched engine", () => {
    // 5.4's third category. "Unsupported" would understate these: the surfaces *are* spoofed,
    // just not from the stored value.
    expect([...spoofedNotConfigurableFingerprintFields("veil-patched")].sort()).toEqual([
      "audioNoise",
      "canvasNoise",
    ]);
  });

  it("gives the View one answer per field, and keeps the noise selects live", () => {
    // `off` reaches a patched engine as a `--disable-spoofing=` key, so the switch half of the
    // control is real even though the level is not — that is why editability is data and not a
    // rule the View gets to invent.
    expect(fingerprintFieldState("veil-patched", "seed")).toEqual({
      support: "settable",
      editable: true,
      noteKey: null,
    });
    expect(fingerprintFieldState("veil-patched", "canvasNoise")).toEqual({
      support: "spoofedNotConfigurable",
      editable: true,
      noteKey: "fingerprint.field.spoofedNotConfigurable.canvasNoise",
    });
    // T15 moved this one out of the third category: `905` reads
    // `--fingerprint-device-memory`, so the stored value is the answer.
    expect(fingerprintFieldState("veil-patched", "deviceMemory")).toEqual({
      support: "settable",
      editable: true,
      noteKey: null,
    });
    expect(fingerprintFieldState("veil-patched", "fonts")).toEqual({
      support: "unsupported",
      editable: false,
      noteKey: "fingerprint.field.unsupported",
    });
    // Timezone is the field the flip makes real: dead under stock, settable under veil-patched.
    expect(fingerprintFieldState("stock-chromium", "timezone").support).toBe("unsupported");
    expect(fingerprintFieldState("veil-patched", "timezone").support).toBe("settable");
    // T22 moved `location` the same way: `970` reads `--fingerprint-location`, and the surface
    // was measured before this mirror gained the field (§1's order).
    expect(fingerprintFieldState("stock-chromium", "location").support).toBe("unsupported");
    expect(fingerprintFieldState("veil-patched", "location")).toEqual({
      support: "settable",
      editable: true,
      noteKey: null,
    });
    // The display block is T06's: `960` reads all three, so they move the same way timezone did.
    for (const field of ["panelWidth", "panelHeight", "devicePixelRatio"] as const) {
      expect(fingerprintFieldState("stock-chromium", field).support, field).toBe("unsupported");
      expect(fingerprintFieldState("veil-patched", field), field).toEqual({
        support: "settable",
        editable: true,
        noteKey: null,
      });
    }
  });

  it("names a note key for every field it does not call settable", () => {
    // The key strings are asserted literally here and their existence is asserted literally in
    // `@veil/i18n`'s own suite (`carries a string for every fingerprint field note`). Splitting it
    // that way keeps `@veil/shared` free of a dependency on the catalog while still failing if
    // either side drifts — a missing entry renders the key itself in front of a user.
    const dialects: FlagDialect[] = ["stock-chromium", "veil-patched"];
    const fields: DialectGatedFingerprintField[] = [
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
    ];
    const keys = new Set<string>();
    for (const dialect of dialects) {
      for (const field of fields) {
        const { support, noteKey } = fingerprintFieldState(dialect, field);
        expect(support === "settable", `${dialect}/${field}`).toBe(noteKey === null);
        if (noteKey) keys.add(noteKey);
      }
    }
    expect([...keys].sort()).toEqual([
      "fingerprint.field.spoofedNotConfigurable.audioNoise",
      "fingerprint.field.spoofedNotConfigurable.canvasNoise",
      "fingerprint.field.unsupported",
    ]);
  });

  it("hands the user agent to the seed once one is set, under either dialect", () => {
    // Mirrors `veil_core::validate`, which rejects the pair. It is deliberately not a
    // `FingerprintFieldState`: the rule depends on another field's value, not on the dialect, because
    // `validate` takes no dialect so a stored profile survives an engine swap.
    expect(seedOwnsUserAgent(emptyFingerprint())).toBe(false);
    expect(seedOwnsUserAgent({ ...emptyFingerprint(), seed: 7 })).toBe(true);
  });

  /**
   * T24-b (6.26): the picker offers four values and the union keeps five. The second half of the
   * assertion is what makes that safe — a stored `ios` profile still type-checks, still parses,
   * and is still *not* offered, while `android` is offered again and required to carry its phone
   * coherence.
   */
  it("offers four OS values while the union keeps the one the picker dropped", () => {
    expect([...SELECTABLE_PROFILE_OS]).toEqual(["windows", "macos", "linux", "android"]);
    for (const os of SELECTABLE_PROFILE_OS) {
      expect(isSelectableProfileOs(os), os).toBe(true);
    }
    for (const os of ["ios"] as const) {
      expect(isSelectableProfileOs(os), os).toBe(false);
      // Still a member of the union, which is the whole point: the type is not narrowed, only the
      // control. `tsc` would reject the line below if `ios` had left `ProfileOS`.
      const stillAssignable: ProfileOS = os;
      expect(stillAssignable).toBe(os);
    }
    // The android requirement, mirrored from `veil_core::validate`: a phone has a touchscreen.
    expect(androidRequiresTouchPoints("android")).toBe(true);
    expect(androidRequiresTouchPoints("windows")).toBe(false);
    expect(androidRequiresTouchPoints("ios")).toBe(false);
  });

  /**
   * The platform gate, and the one place this mirror and the engine can disagree: `to_engine_args`
   * and `validate` both read `ProfileOs::has_fingerprint_platform`, and an ios profile gets no
   * `--fingerprint-platform` at all, so the display block derived from it is unreachable on *any*
   * engine. Calling it settable would be the 5.4 understatement in the other direction — it would
   * promise control over a surface nothing applies. Android left this gate when the port landed:
   * the display block is not just settable there, it is *required* by `validate` — which the UI
   * surfaces through the save path, not through a field state.
   */
  it("closes the display block on an OS that claims no platform", () => {
    for (const field of ["panelWidth", "panelHeight", "devicePixelRatio"] as const) {
      expect(fingerprintFieldState("veil-patched", field, "ios"), `ios/${field}`).toEqual({
        support: "unsupported",
        editable: false,
        noteKey: "fingerprint.field.displayNeedsPlatform",
      });
    }
    // The other two kinds of gate keep their own answers on a platform-less profile: a seed still
    // reaches the engine, and the touch count is not platform-derived.
    expect(fingerprintFieldState("veil-patched", "seed", "ios").support).toBe("settable");
    expect(fingerprintFieldState("veil-patched", "maxTouchPoints", "ios").support).toBe("settable");
    // Where both gates apply the dialect wins, so the note names the more fundamental reason.
    expect(fingerprintFieldState("stock-chromium", "panelWidth", "ios").noteKey).toBe(
      "fingerprint.field.unsupported",
    );
    // Android: the display block is settable again (6.26) — the engine answers the claim, and
    // `validate` requires the block rather than refusing it.
    for (const field of ["panelWidth", "panelHeight", "devicePixelRatio"] as const) {
      expect(fingerprintFieldState("veil-patched", field, "android").support, field).toBe(
        "settable",
      );
    }
    // The desktop control: the same field and dialect stay settable, so this is about the OS.
    for (const os of ["windows", "macos", "linux"]) {
      expect(fingerprintFieldState("veil-patched", "panelWidth", os).support, os).toBe("settable");
    }
    // No OS given is the pre-T24 behaviour, which every other caller and test still relies on.
    expect(fingerprintFieldState("veil-patched", "panelWidth").support).toBe("settable");
  });
});
