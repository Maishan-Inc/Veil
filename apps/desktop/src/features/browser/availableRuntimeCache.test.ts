import { afterEach, describe, expect, it } from "vitest";
import type { AvailableRuntime } from "@veil/shared";
import {
  clearAvailableRuntimeCacheForTests,
  getCachedAvailableRuntimes,
  rememberAvailableRuntimes,
} from "./availableRuntimeCache";

const runtime: AvailableRuntime = {
  version: "151.0.7922.137-1.1",
  assetName: "ungoogled-chromium_151.0.7922.137-1.1_windows_x64.zip",
  downloadUrl: "https://example.invalid/ungoogled-chromium_151.0.7922.137-1.1_windows_x64.zip",
  sha256: "a".repeat(64),
  sizeBytes: 42,
  publishedAt: "2026-06-30T00:00:00Z",
};

describe("available runtime cache", () => {
  afterEach(() => {
    clearAvailableRuntimeCacheForTests();
  });

  it("starts empty", () => {
    expect(getCachedAvailableRuntimes()).toBeNull();
  });

  it("keeps the last successful available runtime list", () => {
    rememberAvailableRuntimes([runtime]);

    expect(getCachedAvailableRuntimes()).toEqual([runtime]);
  });

  it("returns copies so callers cannot mutate the cache", () => {
    rememberAvailableRuntimes([runtime]);

    getCachedAvailableRuntimes()?.pop();

    expect(getCachedAvailableRuntimes()).toEqual([runtime]);
  });

  it("restores the last successful list from local storage", () => {
    rememberAvailableRuntimes([runtime]);

    clearOnlyMemoryCacheForTest();

    expect(getCachedAvailableRuntimes()).toEqual([runtime]);
  });

  it("ignores invalid local storage entries", () => {
    localStorage.setItem("veil.browser.availableRuntimes.v1", JSON.stringify([{ version: 120 }]));

    expect(getCachedAvailableRuntimes()).toBeNull();
  });
});

function clearOnlyMemoryCacheForTest(): void {
  const stored = localStorage.getItem("veil.browser.availableRuntimes.v1");
  clearAvailableRuntimeCacheForTests();
  if (stored) localStorage.setItem("veil.browser.availableRuntimes.v1", stored);
}
