import { describe, expect, it } from "vitest";
import type { RuntimeProgress } from "@veil/shared";
import type { EngineInstallJob } from "../../app/engineInstallStore";
import { buildInstallDockView } from "./installDock";

const candidate = {
  version: "151.0.7922.137-1.1001",
  assetName: "ungoogled-chromium_151.0.7922.137-1.1001_windows_x64.zip",
  downloadUrl: "https://example.invalid/engine.zip",
  sha256: "ad35529a",
  sizeBytes: 205_007_900,
  publishedAt: null,
};

function job(progress: RuntimeProgress | null, errorCode: EngineInstallJob["errorCode"] = null) {
  return {
    target: { source: "remote" as const, candidate },
    progress,
    errorCode,
  } satisfies EngineInstallJob;
}

describe("buildInstallDockView", () => {
  it("carries the download percentage once a total is known", () => {
    const view = buildInstallDockView(
      job({ kind: "downloading", downloaded: 50, total: 200, bytesPerSec: 1_000 }),
    );
    expect(view.percent).toBe(25);
    expect(view.detailKey).toBe("browser.progress.downloadingDetail");
    expect(view.action).toBe("cancel");
    expect(view.pulse).toBe(true);
  });

  /**
   * The state the user reported as frozen: an install that had started but produced no bytes, which
   * the old dialog rendered as `Downloading… 0 B · —/s`. A byte count and a rate that are both
   * placeholders are worse than saying nothing, so this reads as preparing until data arrives.
   */
  it("says preparing while zero bytes have arrived", () => {
    const view = buildInstallDockView(
      job({ kind: "downloading", downloaded: 0, total: null, bytesPerSec: 0 }),
    );
    expect(view.percent).toBeNull();
    expect(view.detailKey).toBe("browser.dock.preparing");
    expect(view.pulse).toBe(true);
  });

  /** Bytes flowing but no `Content-Length`: report the bytes, keep the ring indeterminate. */
  it("goes indeterminate when the server sent no Content-Length", () => {
    const view = buildInstallDockView(
      job({ kind: "downloading", downloaded: 4_096, total: null, bytesPerSec: 2_048 }),
    );
    expect(view.percent).toBeNull();
    expect(view.detailKey).toBe("browser.progress.downloadingUnknownTotal");
    expect(view.pulse).toBe(true);
  });

  it("measures extraction in zip entries, not bytes", () => {
    const view = buildInstallDockView(
      job({ kind: "extracting", entriesDone: 38, entriesTotal: 76 }),
    );
    expect(view.percent).toBe(50);
    expect(view.detailParams).toEqual({ percent: 50 });
  });

  it("has no percentage for the stages that report no total", () => {
    for (const progress of [{ kind: "verifying" }, { kind: "finalizing" }] as RuntimeProgress[]) {
      const view = buildInstallDockView(job(progress));
      expect(view.percent, progress.kind).toBeNull();
      expect(view.pulse, progress.kind).toBe(true);
    }
  });

  it("shows preparing before the first event arrives", () => {
    expect(buildInstallDockView(job(null)).detailKey).toBe("browser.dock.preparing");
  });

  it("stops pulsing and offers a retry once it failed", () => {
    const view = buildInstallDockView(job(null, "runtime.networkUnavailable"));
    expect(view.tone).toBe("error");
    expect(view.action).toBe("retry");
    expect(view.pulse).toBe(false);
    expect(view.detailKey).toBe("errors.runtime.networkUnavailable");
  });

  /** An error code on the job wins over whatever stage the last progress event named. */
  it("prefers the error over a stale stage", () => {
    const view = buildInstallDockView(
      job({ kind: "downloading", downloaded: 10, total: 20, bytesPerSec: 1 }, "app.unknown"),
    );
    expect(view.detailKey).toBe("errors.app.unknown");
    expect(view.percent).toBeNull();
  });
});
