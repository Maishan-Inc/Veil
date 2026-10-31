import type { InstalledRuntimes, RuntimeVersion } from "@veil/shared";

export interface InstalledVersionListItem {
  runtime: RuntimeVersion;
  isCurrent: boolean;
}

export function buildInstalledVersionList(
  installed: InstalledRuntimes,
): InstalledVersionListItem[] {
  return installed.installed.map((runtime) => ({
    runtime,
    isCurrent: runtime.version === installed.currentVersion,
  }));
}
