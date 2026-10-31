import { describe, expect, it } from "vitest";
import { emptyInstalledRuntimes, type RuntimeVersion } from "@veil/shared";
import { buildInstalledVersionList } from "./installedVersions";

function runtime(version: string): RuntimeVersion {
  return {
    version,
    channel: "free",
    executablePath: `C:/Veil/runtime/${version}/chrome.exe`,
    installedAt: "2026-06-30T00:00:00Z",
    sha256: "a".repeat(64),
    sizeBytes: 42,
  };
}

describe("installed runtime list", () => {
  it("includes the current runtime in the installed versions list", () => {
    const items = buildInstalledVersionList({
      ...emptyInstalledRuntimes(),
      currentVersion: "120",
      installed: [runtime("120")],
    });

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      runtime: { version: "120" },
      isCurrent: true,
    });
  });

  it("marks only the current runtime as current", () => {
    const items = buildInstalledVersionList({
      ...emptyInstalledRuntimes(),
      currentVersion: "120",
      installed: [runtime("119"), runtime("120")],
    });

    expect(items.map((item) => [item.runtime.version, item.isCurrent])).toEqual([
      ["119", false],
      ["120", true],
    ]);
  });
});
