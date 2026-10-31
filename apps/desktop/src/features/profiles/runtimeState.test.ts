import { describe, expect, it } from "vitest";
import { emptyFingerprint, type Profile } from "@veil/shared";
import {
  applyProfileRuntimeOverlays,
  launchFailureCode,
  shouldPollProfileRuntime,
} from "./runtimeState";

const baseProfile: Profile = {
  id: "profile-1",
  name: "Alpha",
  avatarColor: "#5640c9",
  groupId: null,
  tagIds: [],
  proxyId: null,
  os: "windows",
  platform: "Storefront",
  browserVersion: "120",
  notes: "",
  status: "idle",
  startupPage: "veil",
  startupUrl: "",
  fingerprint: emptyFingerprint(),
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  lastLaunchedAt: null,
};

describe("profile runtime state helpers", () => {
  it("overlays transient launch status without mutating the source profile", () => {
    const result = applyProfileRuntimeOverlays([baseProfile], {
      "profile-1": { status: "launching", errorCode: null },
    });

    expect(result[0]?.status).toBe("launching");
    expect(baseProfile.status).toBe("idle");
  });

  it("keeps the original profile when no overlay exists", () => {
    const result = applyProfileRuntimeOverlays([baseProfile], {});

    expect(result[0]).toBe(baseProfile);
  });

  it("extracts launch failure codes", () => {
    expect(
      launchFailureCode({
        kind: "failed",
        profileId: "profile-1",
        code: "launch.runtimeMissing",
      }),
    ).toBe("launch.runtimeMissing");
    expect(
      launchFailureCode({
        kind: "started",
        profileId: "profile-1",
        processId: 123,
      }),
    ).toBeNull();
  });

  it("polls only while profiles are launching or running", () => {
    expect(shouldPollProfileRuntime([baseProfile])).toBe(false);
    expect(shouldPollProfileRuntime([{ ...baseProfile, status: "error" }])).toBe(false);
    expect(shouldPollProfileRuntime([{ ...baseProfile, status: "launching" }])).toBe(true);
    expect(shouldPollProfileRuntime([{ ...baseProfile, status: "running" }])).toBe(true);
  });
});
