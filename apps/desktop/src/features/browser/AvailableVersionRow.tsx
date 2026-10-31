import { useT } from "@veil/i18n";
import type { AvailableRuntime } from "@veil/shared";
import { Button } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import { formatBytes, formatTimestamp } from "./format";

interface AvailableVersionRowProps {
  candidate: AvailableRuntime;
  alreadyInstalled: boolean;
  onInstall: () => void;
}

/** A single available-version row before any install has been started. */
export function AvailableVersionRow({
  candidate,
  alreadyInstalled,
  onInstall,
}: AvailableVersionRowProps) {
  const t = useT();
  return (
    <article className="browser-row">
      <div className="browser-row__title">
        <span className="browser-row__name">{candidate.version}</span>
        <span className="browser-row__meta">
          {formatBytes(candidate.sizeBytes)}
          {candidate.publishedAt
            ? ` · ${t("browser.meta.publishedAt")} ${formatTimestamp(candidate.publishedAt)}`
            : ""}
        </span>
      </div>
      <div className="browser-row__actions">
        <Button
          size="sm"
          variant="primary"
          onClick={onInstall}
          disabled={alreadyInstalled}
          title={alreadyInstalled ? t("errors.runtime.alreadyInstalled") : undefined}
        >
          <Icon name="import" size={14} />
          {t("browser.action.install")}
        </Button>
      </div>
    </article>
  );
}
