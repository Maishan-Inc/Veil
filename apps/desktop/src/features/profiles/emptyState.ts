import { runtimeNeedsInstall, type RuntimeInstallState } from "@veil/shared";

export type ProfilesEmptyAction = "openKernel" | "createProfile";

export interface ProfilesEmptyState {
  titleKey: string;
  bodyKey: string;
  actionKey: string;
  action: ProfilesEmptyAction;
}

export function buildProfilesEmptyState(runtime: RuntimeInstallState | null): ProfilesEmptyState {
  if (runtime && runtimeNeedsInstall(runtime)) {
    return {
      titleKey: "profiles.empty.noKernel.title",
      bodyKey: "profiles.empty.noKernel.body",
      actionKey: "profiles.banner.engineMissing.action",
      action: "openKernel",
    };
  }
  return {
    titleKey: "profiles.empty.title",
    bodyKey: "profiles.empty.body",
    actionKey: "profiles.create",
    action: "createProfile",
  };
}
