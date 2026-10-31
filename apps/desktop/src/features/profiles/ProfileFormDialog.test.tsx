import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@veil/i18n";
import { emptyFingerprint, emptyInstalledRuntimes, type Profile } from "@veil/shared";
import { ProfileFormDialog } from "./ProfileFormDialog";

// The dialog loads its supporting lists on open; stub them so it can render without a Tauri host.
vi.mock("../../api/client", () => ({
  AppError: class AppError extends Error {
    code = "app.unknown";
  },
  createProfile: vi.fn(),
  updateProfile: vi.fn(),
  listGroups: vi.fn(async () => []),
  listInstalledRuntimes: vi.fn(async () => emptyInstalledRuntimes()),
  listProxies: vi.fn(async () => []),
  optimizeFingerprintForProxy: vi.fn(),
  randomizeFingerprint: vi.fn(async () => emptyFingerprint()),
}));

/** A profile stored under the old picker, before `ios` stopped being offered. */
const storedIos: Profile = {
  id: "profile-ios",
  name: "Stored iOS",
  avatarColor: "#5640C9",
  groupId: null,
  tagIds: [],
  proxyId: null,
  os: "ios",
  platform: "",
  browserVersion: "151",
  notes: "",
  status: "idle",
  startupPage: "veil",
  startupUrl: "",
  fingerprint: emptyFingerprint(),
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-01T00:00:00Z",
  lastLaunchedAt: null,
};

function renderDialog(profile?: Profile) {
  return render(
    <I18nProvider locale="en-US">
      <ProfileFormDialog
        open
        onClose={() => undefined}
        onSaved={() => undefined}
        profile={profile}
      />
    </I18nProvider>,
  );
}

/** The OS control's option values, in the order the picker renders them. */
async function osOptionValues(): Promise<string[]> {
  const select = (await screen.findByLabelText("OS fingerprint")) as HTMLSelectElement;
  return Array.from(select.options).map((option) => option.value);
}

describe("ProfileFormDialog OS picker", () => {
  /**
   * T24-b's acceptance (6.26), at the DOM layer rather than at the constant: the picker offers the
   * three desktop values plus android — the port landed — and still not ios, because the control
   * does not carry the value. Asserting on `SELECTABLE_PROFILE_OS` alone would leave the option
   * list free to drift back to literals.
   */
  it("offers four OS values and not the one nothing implements", async () => {
    renderDialog();

    expect(await osOptionValues()).toEqual(["windows", "macos", "linux", "android"]);
  });

  /**
   * The other half: a profile stored before the change must still show what it is. The stored
   * `ios` value is rendered — named, and disable-selected — so the control never goes blank, and
   * it stays unselectable so the value cannot be chosen again. A stored android profile needs no
   * such half-state any more: the picker offers android like any desktop value (6.26).
   */
  it("shows a stored ios value without offering it again", async () => {
    renderDialog(storedIos);

    const select = (await screen.findByLabelText("OS fingerprint")) as HTMLSelectElement;
    expect(Array.from(select.options).map((option) => option.value)).toEqual([
      "ios",
      "windows",
      "macos",
      "linux",
      "android",
    ]);
    expect(select.value).toBe("ios");

    const stored = Array.from(select.options).find((option) => option.value === "ios");
    expect(stored?.disabled).toBe(true);
    // The four the picker does offer stay enabled, which is the way out of the state.
    for (const value of ["windows", "macos", "linux", "android"]) {
      const option = Array.from(select.options).find((candidate) => candidate.value === value);
      expect(option?.disabled, value).toBe(false);
    }
  });
});
