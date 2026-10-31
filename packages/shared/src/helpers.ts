import type { Profile, ProfileStatus } from "./profiles";
import type { RuntimeInstallState } from "./runtime";

/** A runtime needs installing when no engine has been selected as current. */
export function runtimeNeedsInstall(state: RuntimeInstallState): boolean {
  return state.kind === "missing";
}

/** A profile can be launched when it has a name and is not already launching. */
export function isLaunchableProfile(profile: Profile): boolean {
  return profile.name.trim().length > 0 && profile.status !== "launching";
}

export type StatusTone = "success" | "info" | "neutral" | "error";

/** Maps a profile status to a semantic tone for badges/indicators. */
export function profileStatusTone(status: ProfileStatus): StatusTone {
  switch (status) {
    case "running":
      return "success";
    case "launching":
      return "info";
    case "error":
      return "error";
    case "idle":
      return "neutral";
  }
}
