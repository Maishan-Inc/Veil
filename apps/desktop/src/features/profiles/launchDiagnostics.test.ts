import { describe, expect, it } from "vitest";
import {
  emptyFingerprint,
  emptyInstalledRuntimes,
  type InstalledRuntimes,
  type Profile,
  type Proxy,
} from "@veil/shared";
import { buildLaunchDiagnostics } from "./launchDiagnostics";

const installed: InstalledRuntimes = {
  currentVersion: "120",
  engineName: "ungoogled-chromium",
  flagDialect: "stock-chromium",
  dialectRecorded: true,
  installed: [
    {
      version: "120",
      channel: "free",
      executablePath: "C:/Veil/runtime/120/chrome.exe",
      installedAt: "2026-06-30T00:00:00Z",
      sha256: "a".repeat(64),
      sizeBytes: 42,
    },
    {
      version: "119",
      channel: "free",
      executablePath: "C:/Veil/runtime/119/chrome.exe",
      installedAt: "2026-06-29T00:00:00Z",
      sha256: "b".repeat(64),
      sizeBytes: 41,
    },
  ],
};

const profile: Profile = {
  id: "profile-1",
  name: "Alpha",
  avatarColor: "#5640c9",
  groupId: null,
  tagIds: [],
  proxyId: null,
  os: "windows",
  platform: "Storefront",
  browserVersion: "119",
  notes: "",
  status: "idle",
  startupPage: "veil",
  startupUrl: "",
  fingerprint: emptyFingerprint(),
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  lastLaunchedAt: null,
};

const proxy: Proxy = {
  id: "proxy-1",
  name: "Office",
  type: "http",
  host: "127.0.0.1",
  port: 8080,
  username: "user",
  hasPassword: true,
  country: null,
  detectedIp: null,
  countryCode: null,
  region: null,
  city: null,
  timezone: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

describe("launch diagnostics", () => {
  it("reports the selected target kernel when it is installed", () => {
    expect(buildLaunchDiagnostics(profile, installed, null)).toMatchObject({
      preflightError: null,
      kernelVersion: "119",
      kernelHint: null,
    });
  });

  it("reports missing kernel before launch when none is installed", () => {
    expect(buildLaunchDiagnostics(profile, emptyInstalledRuntimes(), null)).toMatchObject({
      preflightError: "launch.runtimeMissing",
      kernelVersion: null,
      kernelHint: "missing",
    });
  });

  it("falls back to the current kernel when the target version is unavailable", () => {
    expect(
      buildLaunchDiagnostics({ ...profile, browserVersion: "118" }, installed, null),
    ).toMatchObject({
      preflightError: null,
      kernelVersion: "120",
      kernelHint: "fallback",
    });
  });

  it("flags proxy auth and fingerprint seed launch details", () => {
    expect(
      buildLaunchDiagnostics(
        {
          ...profile,
          proxyId: proxy.id,
          fingerprint: { ...emptyFingerprint(), seed: 123 },
        },
        installed,
        proxy,
      ),
    ).toMatchObject({
      proxyHint: "authIgnored",
      fingerprintSeed: 123,
    });
  });
});
