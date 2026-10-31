import { check } from "@tauri-apps/plugin-updater";

export interface AppUpdate {
  version: string;
  install: () => Promise<void>;
}

/** Called only from the Kernel page's explicit application-update action. */
export async function checkAppUpdate(): Promise<AppUpdate | null> {
  const update = await check();
  if (!update) return null;
  return {
    version: update.version,
    install: () => update.downloadAndInstall(),
  };
}
