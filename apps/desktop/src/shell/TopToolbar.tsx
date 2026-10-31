import type { ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useT } from "@veil/i18n";
import { IconButton, TextField } from "@veil/ui";
import { isTauriEnvironment } from "../api/client";
import { useAppStore } from "../app/store";
import logoUrl from "../assets/veil-logo.png";
import { Icon } from "./Icon";

/** Top toolbar: brand mark, search, primary create + theme toggle. */
export function TopToolbar({ onCreateProfile }: { onCreateProfile: () => void }): ReactNode {
  const t = useT();
  const { search, setSearch, themePreference, setThemePreference } = useAppStore();
  const isDark = themePreference === "dark";

  function toggleTheme(): void {
    setThemePreference(isDark ? "light" : "dark");
  }

  function minimizeWindow(): void {
    if (isTauriEnvironment()) void getCurrentWindow().minimize();
  }

  async function toggleFullscreenWindow(): Promise<void> {
    if (!isTauriEnvironment()) return;
    const currentWindow = getCurrentWindow();
    await currentWindow.setFullscreen(!(await currentWindow.isFullscreen()));
  }

  function closeWindow(): void {
    if (isTauriEnvironment()) void getCurrentWindow().close();
  }

  return (
    <header className="veil-topbar">
      <div className="veil-topbar__brand">
        <img className="veil-topbar__brand-mark" src={logoUrl} alt="" aria-hidden="true" />
        <span>{t("app.name")}</span>
      </div>

      <div className="veil-topbar__search">
        <TextField
          type="search"
          placeholder={t("toolbar.search")}
          value={search}
          onChange={(event) => setSearch(event.currentTarget.value)}
          aria-label={t("toolbar.search")}
        />
      </div>

      <div className="veil-topbar__actions">
        <IconButton label={t("toolbar.toggleTheme")} onClick={toggleTheme}>
          <Icon name={isDark ? "sun" : "moon"} size={18} />
        </IconButton>
        <IconButton label={t("toolbar.createProfile")} onClick={onCreateProfile}>
          <Icon name="plus" size={18} />
        </IconButton>
        <div className="veil-window-controls" aria-label="Window controls">
          <IconButton label={t("window.minimize")} onClick={minimizeWindow}>
            <Icon name="minus" size={17} />
          </IconButton>
          <IconButton label={t("window.fullscreen")} onClick={() => void toggleFullscreenWindow()}>
            <Icon name="square" size={15} />
          </IconButton>
          <IconButton
            className="veil-window-controls__close"
            label={t("window.close")}
            onClick={closeWindow}
          >
            <Icon name="x" size={17} />
          </IconButton>
        </div>
      </div>
    </header>
  );
}
