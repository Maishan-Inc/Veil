import React from "react";
import ReactDOM from "react-dom/client";
import { applyTheme, loadThemePreference, watchSystemTheme } from "@veil/ui";
import "@veil/ui/styles/tokens.css";
import "@veil/ui/styles/global.css";
import "@veil/ui/styles/components.css";
import "./styles/app.css";
import { App } from "./app/App";
import { I18nProvider } from "@veil/i18n";
import { useAppStore } from "./app/store";

// Apply theme as early as possible so the first paint matches the user's preference.
const initialPref = loadThemePreference();
applyTheme(initialPref);

// Keep auto-mode in sync with the OS.
watchSystemTheme(() => {
  if (useAppStore.getState().themePreference === "auto") {
    applyTheme("auto");
  }
});

function Root(): React.ReactElement {
  const locale = useAppStore((state) => state.locale);
  return (
    <I18nProvider locale={locale}>
      <App />
    </I18nProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
