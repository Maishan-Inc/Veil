import { describe, expect, it } from "vitest";
import { emptyInstalledRuntimes, type InstalledRuntimes } from "@veil/shared";
import { buildRuntimeReadiness } from "./runtimeReadiness";

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
  ],
};

describe("runtime readiness", () => {
  it("marks the current runtime ready", () => {
    const steps = buildRuntimeReadiness({
      installed,
      availableState: "notLoaded",
      availableCount: 0,
      localZipPath: "",
    });

    expect(steps[0]).toMatchObject({
      id: "kernel",
      tone: "success",
      params: { version: "120" },
    });
  });

  it("asks for a default runtime when versions exist but none is current", () => {
    const steps = buildRuntimeReadiness({
      installed: { ...installed, currentVersion: null },
      availableState: "notLoaded",
      availableCount: 0,
      localZipPath: "",
    });

    expect(steps[0]).toMatchObject({
      id: "kernel",
      tone: "info",
      detailKey: "browser.readiness.kernel.selectDefault",
      params: { count: 1 },
    });
  });

  it("marks release history and local zip as ready when both are present", () => {
    const steps = buildRuntimeReadiness({
      installed: emptyInstalledRuntimes(),
      availableState: "idle",
      availableCount: 2,
      localZipPath: "D:/Downloads/ungoogled-chromium_151.0.7922.137-1.1_windows_x64.zip",
    });

    expect(steps[1]).toMatchObject({
      id: "history",
      tone: "success",
      params: { count: 2 },
    });
    expect(steps[2]).toMatchObject({
      id: "localZip",
      tone: "success",
      detailKey: "browser.readiness.localZip.ready",
    });
  });

  it("surfaces history load errors as attention", () => {
    const steps = buildRuntimeReadiness({
      installed: emptyInstalledRuntimes(),
      availableState: "error",
      availableCount: 0,
      localZipPath: "",
    });

    expect(steps[1]).toMatchObject({
      id: "history",
      tone: "warning",
      statusKey: "browser.readiness.status.attention",
    });
  });
});
