import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@veil/i18n";
import { emptyFingerprint, type FlagDialect, type ProfileOS } from "@veil/shared";
import { FingerprintSection } from "./FingerprintSection";

// The section only calls the API when the user clicks "Randomize"; stub it so the component can
// render without a Tauri host.
vi.mock("../../api/client", () => ({
  randomizeFingerprint: vi.fn(async () => emptyFingerprint()),
}));

/** Fields that only exist on an engine carrying the Veil patch set, by visible label. */
const PATCH_ONLY_LABELS = [
  "Fingerprint seed",
  "Timezone",
  "Display panel",
  "Device pixel ratio",
  "WebGL vendor",
  "WebGL renderer",
  "Canvas noise",
  "Audio noise",
  "Fonts",
  "CPU cores",
  "Memory (GB)",
  "Touch points",
  "Location",
];

/** Fields upstream Chromium implements, so they stay editable under every dialect. */
const ALWAYS_SUPPORTED_LABELS = [
  "User agent",
  "Window size",
  "Locale",
  "Languages",
  "WebRTC policy",
];

/**
 * How a patched engine treats each of `PATCH_ONLY_LABELS`.
 * Three states, not two: the labels absent from both lists below are directly settable.
 */
const PATCHED_UNSUPPORTED_LABELS = ["WebGL vendor", "WebGL renderer", "Fonts"];
/** Spoofed from the seed. Editable where part of the value still lands (`off`), disabled where not. */
const PATCHED_SPOOFED_EDITABLE_LABELS = ["Canvas noise", "Audio noise"];
/**
 * Empty since Stage 6 T15. "Memory (GB)" was the only member: `005` reported 8 whatever the profile
 * said, and `905` replaced that body with a `--fingerprint-device-memory` read, so the field is
 * settable and nothing is left in this state that is also disabled.
 */
const PATCHED_SPOOFED_DISABLED_LABELS: string[] = [];
/**
 * Settable on a patched engine, but only once the profile has a seed: `960` derives the work area,
 * the colour depth and the orientation from `--fingerprint-platform`, which `to_engine_args` emits
 * only alongside one, so `veil_core::validate` rejects a seedless panel. The default fingerprint the
 * other tests render has `seed: 0`, which is why these are disabled there.
 */
const PATCHED_NEEDS_SEED_LABELS = ["Display panel", "Device pixel ratio"];

const UNSUPPORTED_NOTE =
  "The installed engine has no switch for this. The value is stored, and nothing applies it.";

function renderSection(
  flagDialect: FlagDialect,
  dialectRecorded = true,
  value = emptyFingerprint(),
  os: ProfileOS = "windows",
) {
  return render(
    <I18nProvider locale="en-US">
      <FingerprintSection
        os={os}
        value={value}
        onChange={() => undefined}
        flagDialect={flagDialect}
        dialectRecorded={dialectRecorded}
        engineName="ungoogled-chromium"
      />
    </I18nProvider>,
  );
}

describe("FingerprintSection", () => {
  it("disables the fields a stock engine cannot honour and explains why", () => {
    renderSection("stock-chromium");

    for (const label of PATCH_ONLY_LABELS) {
      expect(screen.getByLabelText(label), label).toBeDisabled();
    }
    for (const label of ALWAYS_SUPPORTED_LABELS) {
      expect(screen.getByLabelText(label), label).toBeEnabled();
    }
    expect(screen.getAllByText(UNSUPPORTED_NOTE)).toHaveLength(PATCH_ONLY_LABELS.length);
  });

  it("warns that a stock engine provides no fingerprint protection", () => {
    renderSection("stock-chromium");

    expect(screen.getByText("This engine provides no fingerprint protection")).toBeInTheDocument();
    // The banner names the engine so the user knows which install is being described.
    expect(screen.getByText(/ungoogled-chromium is stock Chromium/)).toBeInTheDocument();
  });

  /**
   * An engine installed before Veil recorded capability behaves exactly like stock — that is the only
   * safe direction — but it has never been measured, so the banner must not assert that it cannot
   * spoof. Unknown and known-stock are the same behaviour and different claims (5.5c).
   */
  it("says the capability is unrecorded rather than absent when nothing was recorded", () => {
    renderSection("stock-chromium", false);

    expect(screen.getByText("This engine's capabilities were never recorded")).toBeInTheDocument();
    expect(
      screen.queryByText("This engine provides no fingerprint protection"),
    ).not.toBeInTheDocument();
    // Same gating as stock: the fields a patched engine alone can honour stay disabled.
    for (const label of PATCH_ONLY_LABELS) {
      expect(screen.getByLabelText(label), label).toBeDisabled();
    }
  });

  it("splits a patched engine's fields into settable, spoofed-not-configurable, and unsupported", () => {
    renderSection("veil-patched");

    const settable = [
      ...ALWAYS_SUPPORTED_LABELS,
      ...PATCH_ONLY_LABELS.filter(
        (label) =>
          ![
            ...PATCHED_UNSUPPORTED_LABELS,
            ...PATCHED_SPOOFED_EDITABLE_LABELS,
            ...PATCHED_SPOOFED_DISABLED_LABELS,
            ...PATCHED_NEEDS_SEED_LABELS,
          ].includes(label),
      ),
    ];
    // Seed, Timezone and CPU cores are the three Tier 1 honours as typed.
    expect(settable).toContain("Fingerprint seed");
    expect(settable).toContain("Timezone");
    expect(settable).toContain("CPU cores");

    for (const label of [...settable, ...PATCHED_SPOOFED_EDITABLE_LABELS]) {
      expect(screen.getByLabelText(label), label).toBeEnabled();
    }
    for (const label of [
      ...PATCHED_UNSUPPORTED_LABELS,
      ...PATCHED_SPOOFED_DISABLED_LABELS,
      ...PATCHED_NEEDS_SEED_LABELS,
    ]) {
      expect(screen.getByLabelText(label), label).toBeDisabled();
    }

    // "Unsupported" is reserved for the three fields no switch exists for. Saying it about a
    // spoofed surface understates the engine; saying nothing about it overstates the user's
    // control. Both are the same class of bug as emitting a switch the engine ignores.
    expect(screen.getAllByText(UNSUPPORTED_NOTE)).toHaveLength(PATCHED_UNSUPPORTED_LABELS.length);
    expect(screen.getAllByText(/Low and High are the same instruction/)).toHaveLength(
      PATCHED_SPOOFED_EDITABLE_LABELS.length,
    );
    // The display block gets its own reason, not the unsupported one: the switch exists and the
    // engine reads it, so calling it unsupported would understate `960` the same way calling a
    // spoofed surface unsupported understates the seed.
    expect(screen.getAllByText(/Needs a fingerprint seed/)).toHaveLength(
      PATCHED_NEEDS_SEED_LABELS.length,
    );
  });

  /**
   * The other half of the seed gate. With a seed the display block is settable, and the note is gone —
   * so the disabled state above is about the *profile*, not about the engine's capability.
   */
  it("opens the display block once the profile has a seed", () => {
    renderSection("veil-patched", true, { ...emptyFingerprint(), seed: 424242 });

    for (const label of PATCHED_NEEDS_SEED_LABELS) {
      expect(screen.getByLabelText(label), label).toBeEnabled();
    }
    expect(screen.queryByText(/Needs a fingerprint seed/)).not.toBeInTheDocument();
  });

  /**
   * The third cross-field gate, and the only one keyed on the profile's OS rather than on its seed:
   * macOS builds a Chromium whose `MaxTouchPoints()` is a constant 0, so a Mac profile cannot claim a
   * touchscreen and `veil_core::validate` refuses one. Windows can, so the same control is live there.
   */
  it("closes the touch-point field on a macOS profile and leaves it open elsewhere", () => {
    const { unmount } = renderSection("veil-patched", true, emptyFingerprint(), "macos");

    expect(screen.getByLabelText("Touch points")).toBeDisabled();
    expect(screen.getByText(/A Mac reports no touch points/)).toBeInTheDocument();
    unmount();

    renderSection("veil-patched", true, emptyFingerprint(), "windows");
    expect(screen.getByLabelText("Touch points")).toBeEnabled();
    expect(screen.queryByText(/A Mac reports no touch points/)).not.toBeInTheDocument();
  });

  /**
   * The fourth cross-field gate, now keyed on the one OS that still claims no platform. `ios`
   * stayed out of the picker in T24 because no patch anywhere implements it, so
   * `--fingerprint-platform` is never emitted for it and `960` has no platform to derive the work
   * area, colour depth and orientation from. The display block is therefore unreachable on such a
   * profile — and it says *that*, rather than borrowing the seed's note: this profile has a seed,
   * so naming one would point the user at a fix that does not exist. `android` left this gate in
   * 6.26: the port answers the claim, and the display block is required there rather than closed.
   */
  it("closes the display block on a profile whose OS claims no platform", () => {
    const seeded = { ...emptyFingerprint(), seed: 424242 };
    const { unmount } = renderSection("veil-patched", true, seeded, "ios");

    for (const label of PATCHED_NEEDS_SEED_LABELS) {
      expect(screen.getByLabelText(label), label).toBeDisabled();
    }
    expect(screen.getAllByText(/presents no platform to the engine/)).toHaveLength(
      PATCHED_NEEDS_SEED_LABELS.length,
    );
    expect(screen.queryByText(/Needs a fingerprint seed/)).not.toBeInTheDocument();
    unmount();

    // The android control: the same seeded fingerprint on android opens the block — the port
    // landed, and the display block is what `validate` requires of the claim.
    const androidRender = renderSection("veil-patched", true, seeded, "android");
    for (const label of PATCHED_NEEDS_SEED_LABELS) {
      expect(screen.getByLabelText(label), label).toBeEnabled();
    }
    expect(screen.queryByText(/presents no platform to the engine/)).not.toBeInTheDocument();
    androidRender.unmount();

    // The desktop control: the same seeded fingerprint on Windows opens the block, so the disabled
    // state above is about the platform claim and not about the values or the dialect.
    renderSection("veil-patched", true, seeded, "windows");
    for (const label of PATCHED_NEEDS_SEED_LABELS) {
      expect(screen.getByLabelText(label), label).toBeEnabled();
    }
    expect(screen.queryByText(/presents no platform to the engine/)).not.toBeInTheDocument();
  });

  it("shows no degraded-capability banner on a patched engine", () => {
    renderSection("veil-patched");

    expect(
      screen.queryByText("This engine provides no fingerprint protection"),
    ).not.toBeInTheDocument();
  });
});
