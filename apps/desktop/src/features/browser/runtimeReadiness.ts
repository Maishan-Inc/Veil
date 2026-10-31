import type { InstalledRuntimes } from "@veil/shared";

export type AvailableRuntimeState = "notLoaded" | "idle" | "loading" | "error";
export type RuntimeReadinessStepId = "kernel" | "history" | "localZip";
export type RuntimeReadinessTone = "neutral" | "success" | "warning" | "info";

export interface RuntimeReadinessStep {
  id: RuntimeReadinessStepId;
  titleKey: string;
  detailKey: string;
  statusKey: string;
  tone: RuntimeReadinessTone;
  params?: Record<string, string | number>;
}

export interface RuntimeReadinessInput {
  installed: InstalledRuntimes;
  availableState: AvailableRuntimeState;
  availableCount: number;
  localZipPath: string;
}

export function buildRuntimeReadiness(input: RuntimeReadinessInput): RuntimeReadinessStep[] {
  return [
    kernelStep(input.installed),
    historyStep(input.availableState, input.availableCount),
    localZipStep(input.availableState, input.availableCount, input.localZipPath),
  ];
}

function kernelStep(installed: InstalledRuntimes): RuntimeReadinessStep {
  const current = installed.currentVersion
    ? installed.installed.find((runtime) => runtime.version === installed.currentVersion)
    : null;
  if (current) {
    return {
      id: "kernel",
      titleKey: "browser.readiness.kernel.title",
      detailKey: "browser.readiness.kernel.ready",
      statusKey: "browser.readiness.status.ready",
      tone: "success",
      params: { version: current.version },
    };
  }
  if (installed.installed.length > 0) {
    return {
      id: "kernel",
      titleKey: "browser.readiness.kernel.title",
      detailKey: "browser.readiness.kernel.selectDefault",
      statusKey: "browser.readiness.status.action",
      tone: "info",
      params: { count: installed.installed.length },
    };
  }
  return {
    id: "kernel",
    titleKey: "browser.readiness.kernel.title",
    detailKey: "browser.readiness.kernel.missing",
    statusKey: "browser.readiness.status.attention",
    tone: "warning",
  };
}

function historyStep(
  availableState: AvailableRuntimeState,
  availableCount: number,
): RuntimeReadinessStep {
  if (availableState === "loading") {
    return {
      id: "history",
      titleKey: "browser.readiness.history.title",
      detailKey: "browser.readiness.history.loading",
      statusKey: "browser.readiness.status.action",
      tone: "info",
    };
  }
  if (availableState === "error") {
    return {
      id: "history",
      titleKey: "browser.readiness.history.title",
      detailKey: "browser.readiness.history.error",
      statusKey: "browser.readiness.status.attention",
      tone: "warning",
    };
  }
  if (availableState === "idle" && availableCount > 0) {
    return {
      id: "history",
      titleKey: "browser.readiness.history.title",
      detailKey: "browser.readiness.history.loaded",
      statusKey: "browser.readiness.status.ready",
      tone: "success",
      params: { count: availableCount },
    };
  }
  if (availableState === "idle") {
    return {
      id: "history",
      titleKey: "browser.readiness.history.title",
      detailKey: "browser.readiness.history.empty",
      statusKey: "browser.readiness.status.attention",
      tone: "warning",
    };
  }
  return {
    id: "history",
    titleKey: "browser.readiness.history.title",
    detailKey: "browser.readiness.history.notLoaded",
    statusKey: "browser.readiness.status.pending",
    tone: "neutral",
  };
}

function localZipStep(
  availableState: AvailableRuntimeState,
  availableCount: number,
  localZipPath: string,
): RuntimeReadinessStep {
  if (localZipPath.trim().length > 0) {
    return {
      id: "localZip",
      titleKey: "browser.readiness.localZip.title",
      detailKey: "browser.readiness.localZip.ready",
      statusKey: "browser.readiness.status.ready",
      tone: "success",
    };
  }
  if (availableState === "idle" && availableCount > 0) {
    return {
      id: "localZip",
      titleKey: "browser.readiness.localZip.title",
      detailKey: "browser.readiness.localZip.waitingPath",
      statusKey: "browser.readiness.status.pending",
      tone: "neutral",
    };
  }
  return {
    id: "localZip",
    titleKey: "browser.readiness.localZip.title",
    detailKey: "browser.readiness.localZip.waitingHistory",
    statusKey: "browser.readiness.status.pending",
    tone: "neutral",
  };
}
