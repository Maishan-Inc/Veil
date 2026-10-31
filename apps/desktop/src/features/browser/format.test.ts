import { describe, expect, it } from "vitest";
import { formatBytes, formatRate, percentOf, shortenPath } from "./format";

describe("browser format helpers", () => {
  it("formats byte counts with sensible precision", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
    expect(formatBytes(1024)).toBe("1.00 KB");
    expect(formatBytes(150 * 1024 * 1024)).toBe("150 MB");
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(-1)).toBe("—");
  });

  it("formats rates the same as bytes", () => {
    expect(formatRate(0)).toBe("—");
    expect(formatRate(1024 * 1024)).toBe("1.00 MB");
  });

  it("computes percentages", () => {
    expect(percentOf(0, 100)).toBe(0);
    expect(percentOf(50, 100)).toBe(50);
    expect(percentOf(100, 100)).toBe(100);
    expect(percentOf(150, 100)).toBe(100);
    expect(percentOf(10, null)).toBeNull();
  });

  it("shortens paths from the middle", () => {
    const long = "C:/Users/user/AppData/Local/Veil/runtime/versions/120/engine.exe";
    expect(shortenPath(long, 40)).toContain("…");
    expect(shortenPath("short", 40)).toBe("short");
  });
});
