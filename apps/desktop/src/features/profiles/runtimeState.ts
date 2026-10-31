import type { AppErrorCode, LaunchStatus, Profile, ProfileStatus } from "@veil/shared";

export interface ProfileRuntimeOverlay {
  status: ProfileStatus;
  errorCode: AppErrorCode | null;
}

export type ProfileRuntimeOverlays = Record<string, ProfileRuntimeOverlay | undefined>;

export function applyProfileRuntimeOverlays(
  profiles: Profile[],
  overlays: ProfileRuntimeOverlays,
): Profile[] {
  return profiles.map((profile) => {
    const overlay = overlays[profile.id];
    if (!overlay || overlay.status === profile.status) return profile;
    return { ...profile, status: overlay.status };
  });
}

export function launchFailureCode(result: LaunchStatus): AppErrorCode | null {
  return result.kind === "failed" ? result.code : null;
}

export function shouldPollProfileRuntime(profiles: Profile[]): boolean {
  return profiles.some((profile) => profile.status === "running" || profile.status === "launching");
}
