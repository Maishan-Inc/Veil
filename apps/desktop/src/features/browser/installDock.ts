import type { RuntimeProgress } from "@veil/shared";
import { formatBytes, formatRate, percentOf } from "./format";
import type { EngineInstallJob } from "../../app/engineInstallStore";

export interface InstallDockView {
  /** Ring value 0..100, or `null` while the stage has no measurable total. */
  percent: number | null;
  tone: "accent" | "error";
  /** i18n key + params for the one-line stage description. */
  detailKey: string;
  detailParams: Record<string, string | number>;
  /** Which affordance the dock offers. */
  action: "cancel" | "retry";
  /** Whether the ring breathes. Off once the job has stopped moving. */
  pulse: boolean;
}

/**
 * Map an install job onto what the dock renders. Pure so the state machine can be tested
 * without mounting anything — the same reason `runtimeReadiness` and `installedVersions` are
 * separate from their rows.
 *
 * `percent` is deliberately per-stage rather than a single 0-100 across the whole install:
 * download and extraction have unrelated units (bytes vs zip entries), and averaging them would
 * invent a number the backend never reported.
 */
export function buildInstallDockView(job: EngineInstallJob): InstallDockView {
  if (job.errorCode) {
    return {
      percent: null,
      tone: "error",
      detailKey: `errors.${job.errorCode}`,
      detailParams: {},
      action: "retry",
      pulse: false,
    };
  }
  return { ...stageView(job.progress), tone: "accent", action: "cancel", pulse: true };
}

type StageView = Pick<InstallDockView, "percent" | "detailKey" | "detailParams">;

function stageView(progress: RuntimeProgress | null): StageView {
  if (progress === null) {
    return { percent: null, detailKey: "browser.dock.preparing", detailParams: {} };
  }
  switch (progress.kind) {
    case "downloading": {
      // Before the first chunk there is nothing to report but the fact that Veil is trying.
      // The old dialog rendered this state as `0 B · —/s`, which is what a stuck connect looked
      // like to the user who reported it: a byte count and a rate that were both placeholders.
      if (progress.downloaded <= 0) {
        return { percent: null, detailKey: "browser.dock.preparing", detailParams: {} };
      }
      const percent = percentOf(progress.downloaded, progress.total);
      if (percent === null) {
        return {
          percent: null,
          detailKey: "browser.progress.downloadingUnknownTotal",
          detailParams: {
            downloaded: formatBytes(progress.downloaded),
            rate: formatRate(progress.bytesPerSec),
          },
        };
      }
      return {
        percent,
        detailKey: "browser.progress.downloadingDetail",
        detailParams: {
          downloaded: formatBytes(progress.downloaded),
          total: formatBytes(progress.total ?? 0),
          rate: formatRate(progress.bytesPerSec),
        },
      };
    }
    case "verifying":
      return { percent: null, detailKey: "browser.progress.verifying", detailParams: {} };
    case "extracting":
      return {
        percent: percentOf(progress.entriesDone, progress.entriesTotal),
        detailKey: "browser.progress.extracting",
        detailParams: { percent: percentOf(progress.entriesDone, progress.entriesTotal) ?? 0 },
      };
    case "finalizing":
      return { percent: null, detailKey: "browser.progress.finalizing", detailParams: {} };
    case "completed":
      return { percent: 100, detailKey: "browser.progress.completed", detailParams: {} };
    case "cancelled":
      return { percent: null, detailKey: "browser.progress.cancelled", detailParams: {} };
    case "failed":
      return { percent: null, detailKey: `errors.${progress.code}`, detailParams: {} };
  }
}
