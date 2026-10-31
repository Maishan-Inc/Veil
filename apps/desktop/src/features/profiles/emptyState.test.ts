import { describe, expect, it } from "vitest";
import { buildProfilesEmptyState } from "./emptyState";

describe("profiles empty state", () => {
  it("points users to the kernel page when no runtime is installed", () => {
    expect(buildProfilesEmptyState({ kind: "missing" })).toMatchObject({
      action: "openKernel",
      titleKey: "profiles.empty.noKernel.title",
    });
  });

  it("keeps create as the primary action when a runtime exists", () => {
    expect(
      buildProfilesEmptyState({
        kind: "installed",
        current: {
          version: "120",
          channel: "free",
          executablePath: "C:/Veil/runtime/120/chrome.exe",
          installedAt: "2026-06-30T00:00:00Z",
          sha256: "a".repeat(64),
          sizeBytes: 42,
        },
      }),
    ).toMatchObject({
      action: "createProfile",
      titleKey: "profiles.empty.title",
    });
  });
});
