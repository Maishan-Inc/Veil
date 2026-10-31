import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Button } from "@veil/ui";
import { Icon } from "../../shell/Icon";

interface BatchBarProps {
  count: number;
  onStart: () => void;
  onStop: () => void;
  onDelete: () => void;
  onModifyProxy: () => void;
  onExport: () => void;
  onClear: () => void;
}

/** Inline strip that appears when one or more profiles are selected. */
export function BatchBar({
  count,
  onStart,
  onStop,
  onDelete,
  onModifyProxy,
  onExport,
  onClear,
}: BatchBarProps): ReactNode {
  const t = useT();
  return (
    <div className="profiles-batch" role="region" aria-label={t("profiles.selected", { count })}>
      <span className="profiles-batch__label">{t("profiles.selected", { count })}</span>
      <span className="profiles-batch__actions">
        <Button size="sm" variant="secondary" onClick={onStart}>
          <Icon name="play" size={14} />
          {t("profiles.batch.start")}
        </Button>
        <Button size="sm" variant="secondary" onClick={onStop}>
          <Icon name="stop" size={14} />
          {t("profiles.batch.stop")}
        </Button>
        <Button size="sm" variant="secondary" onClick={onModifyProxy}>
          {t("profiles.batch.modifyProxy")}
        </Button>
        <Button size="sm" variant="secondary" onClick={onExport}>
          <Icon name="export" size={14} />
          {t("profiles.batch.export")}
        </Button>
        <Button size="sm" variant="danger" onClick={onDelete}>
          <Icon name="trash" size={14} />
          {t("profiles.batch.delete")}
        </Button>
      </span>
      <span className="profiles-batch__clear">
        <Button size="sm" variant="secondary" onClick={onClear}>
          {t("profiles.clearSelection")}
        </Button>
      </span>
    </div>
  );
}
