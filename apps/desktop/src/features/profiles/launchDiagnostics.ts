import type { AppErrorCode, InstalledRuntimes, Profile, Proxy } from "@veil/shared";

export interface LaunchDiagnostics {
  preflightError: AppErrorCode | null;
  kernelVersion: string | null;
  kernelHint: "missing" | "fallback" | "current" | null;
  proxyHint: "authIgnored" | "missing" | null;
  fingerprintSeed: number | null;
}

export function buildLaunchDiagnostics(
  profile: Profile,
  runtimes: InstalledRuntimes,
  proxy: Proxy | null,
): LaunchDiagnostics {
  const installedVersions = new Set(runtimes.installed.map((runtime) => runtime.version));
  const targetVersion = profile.browserVersion.trim();
  const hasInstalledKernel = runtimes.installed.length > 0;
  const hasTargetKernel = targetVersion.length > 0 && installedVersions.has(targetVersion);
  const fallbackVersion = runtimes.currentVersion ?? runtimes.installed[0]?.version ?? null;

  let kernelVersion: string | null = null;
  let kernelHint: LaunchDiagnostics["kernelHint"] = null;
  let preflightError: AppErrorCode | null = null;

  if (!hasInstalledKernel) {
    kernelHint = "missing";
    preflightError = "launch.runtimeMissing";
  } else if (hasTargetKernel) {
    kernelVersion = targetVersion;
    kernelHint = targetVersion === runtimes.currentVersion ? "current" : null;
  } else if (fallbackVersion) {
    kernelVersion = fallbackVersion;
    kernelHint = targetVersion.length > 0 ? "fallback" : "current";
  }

  return {
    preflightError,
    kernelVersion,
    kernelHint,
    proxyHint: proxyHint(profile, proxy),
    fingerprintSeed: profile.fingerprint.seed > 0 ? profile.fingerprint.seed : null,
  };
}

function proxyHint(profile: Profile, proxy: Proxy | null): LaunchDiagnostics["proxyHint"] {
  if (!profile.proxyId) return null;
  if (!proxy) return "missing";
  return proxy.username || proxy.hasPassword ? "authIgnored" : null;
}
