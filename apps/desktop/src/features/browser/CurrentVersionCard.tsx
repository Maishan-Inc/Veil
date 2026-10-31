import { useT } from "@veil/i18n";
import type { RuntimeVersion } from "@veil/shared";
import { Badge, Button } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import { formatBytes, formatTimestamp, shortenPath } from "./format";

interface CurrentVersionCardProps {
  current: RuntimeVersion | null;
}

/** Highlighted card for the active engine, or an empty-state pitch. */
export function CurrentVersionCard({ current }: CurrentVersionCardProps) {
  const t = useT();

  if (!current) {
    return (
      <section className="browser-card">
        <header className="browser-card__head">
          <h3 className="browser-card__title">{t("browser.current.title")}</h3>
        </header>
        <div className="browser-empty">
          <Icon name="browser" size={28} />
          <div>
            <div className="browser-empty__title">{t("browser.current.empty.title")}</div>
            <p className="browser-empty__body">{t("browser.current.empty.body")}</p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="browser-card browser-card--current">
      <header className="browser-card__head">
        <h3 className="browser-card__title">{t("browser.current.title")}</h3>
        <Badge tone="accent">{t("browser.meta.default")}</Badge>
      </header>
      <div className="browser-card__body">
        <div className="browser-card__row">
          <span className="browser-card__version">{current.version}</span>
          <span className="browser-card__size">{formatBytes(current.sizeBytes)}</span>
        </div>
        <dl className="browser-card__meta">
          <dt>{t("browser.meta.installedAt")}</dt>
          <dd>{formatTimestamp(current.installedAt)}</dd>
          <dt>{t("browser.meta.path")}</dt>
          <dd className="veil-text-code">{shortenPath(current.executablePath, 80)}</dd>
          <dt>{t("browser.meta.checksum")}</dt>
          <dd className="veil-text-code">
            {current.sha256.slice(0, 12)}…{current.sha256.slice(-8)}
          </dd>
        </dl>
      </div>
      <footer className="browser-card__actions">
        <Button size="sm" variant="secondary" disabled>
          <Icon name="folder" size={14} />
          {t("browser.action.openFolder")}
        </Button>
      </footer>
    </section>
  );
}
