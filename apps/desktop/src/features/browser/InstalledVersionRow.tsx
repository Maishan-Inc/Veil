import { useT } from "@veil/i18n";
import type { RuntimeVersion } from "@veil/shared";
import { Badge, Button } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import { formatBytes, formatTimestamp, shortenPath } from "./format";

interface InstalledVersionRowProps {
  runtime: RuntimeVersion;
  isCurrent: boolean;
  onSetDefault: () => void;
  onUninstall: () => void;
}

/** A non-current installed runtime — sits in the "Installed versions" list. */
export function InstalledVersionRow({
  runtime,
  isCurrent,
  onSetDefault,
  onUninstall,
}: InstalledVersionRowProps) {
  const t = useT();
  return (
    <article className="browser-row">
      <div className="browser-row__title">
        <span className="browser-row__name">{runtime.version}</span>
        {isCurrent ? <Badge tone="accent">{t("browser.meta.default")}</Badge> : null}
        <span className="browser-row__meta">
          {formatBytes(runtime.sizeBytes)} · {t("browser.meta.installedAt")}{" "}
          {formatTimestamp(runtime.installedAt)}
        </span>
      </div>
      <div className="browser-row__meta browser-row__meta--path">
        <span className="veil-text-code">{shortenPath(runtime.executablePath, 80)}</span>
      </div>
      <div className="browser-row__actions">
        {!isCurrent ? (
          <Button size="sm" variant="secondary" onClick={onSetDefault}>
            {t("browser.action.setDefault")}
          </Button>
        ) : null}
        <Button
          size="sm"
          variant="subtle"
          onClick={onUninstall}
          disabled={isCurrent}
          title={isCurrent ? t("errors.runtime.cannotUninstallCurrent") : undefined}
        >
          <Icon name="trash" size={14} />
          {t("browser.action.uninstall")}
        </Button>
      </div>
    </article>
  );
}
