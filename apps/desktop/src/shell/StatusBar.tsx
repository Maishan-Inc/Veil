import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { RuntimeInstallState } from "@veil/shared";

interface StatusBarProps {
  runtime: RuntimeInstallState | null;
  profileCount: number;
  isOnline: boolean;
}

export function StatusBar({ runtime, profileCount, isOnline }: StatusBarProps): ReactNode {
  const t = useT();
  const engineLabel =
    runtime?.kind === "installed"
      ? t("status.engine.installed", { version: runtime.current.version })
      : t("status.engine.missing");

  return (
    <footer className="veil-statusbar" role="status">
      <span className="veil-statusbar__item">{engineLabel}</span>
      <span className="veil-statusbar__divider" />
      <span className="veil-statusbar__item">
        {t("status.profileCount", { count: profileCount })}
      </span>
      <span className="veil-statusbar__spacer" />
      <span className="veil-statusbar__item">
        {isOnline ? t("status.ready") : t("status.offline")}
      </span>
    </footer>
  );
}
