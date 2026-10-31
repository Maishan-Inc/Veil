import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { Proxy, ProxyTestResult } from "@veil/shared";
import { Badge, Button, IconButton } from "@veil/ui";
import { Icon } from "../../shell/Icon";
import { formatProxyLocation } from "./format";
import { ProxyTestBadge } from "./ProxyTestBadge";

interface ProxyRowProps {
  proxy: Proxy;
  testState: "idle" | "running" | "result";
  testResult: ProxyTestResult | null;
  onTest: (proxy: Proxy) => void;
  onEdit: (proxy: Proxy) => void;
  onReveal: (proxy: Proxy) => void;
  onDelete: (proxy: Proxy) => void;
}

/**
 * One proxy in the list. Layout (left → right):
 *   name + type chip + auth chip
 *   host:port + country
 *                                              [test result]   [Test] [edit] [delete]
 */
export function ProxyRow({
  proxy,
  testState,
  testResult,
  onTest,
  onEdit,
  onReveal,
  onDelete,
}: ProxyRowProps): ReactNode {
  const t = useT();
  const location = formatProxyLocation(proxy);
  const fallbackAddress = `${proxy.host}:${proxy.port}`;

  return (
    <div className="proxy-row">
      <div className="proxy-row__main">
        <div className="proxy-row__title">
          <span className="proxy-row__name">{proxy.name}</span>
          <Badge tone="info">{t(`proxy.kind.${proxy.type}`)}</Badge>
          {proxy.hasPassword || proxy.username ? (
            <Badge tone="neutral">{t("proxy.field.authShort")}</Badge>
          ) : null}
        </div>
        <div className="proxy-row__meta">
          <span className="proxy-row__addr">{location ?? fallbackAddress}</span>
          {location ? <span className="proxy-row__country">{fallbackAddress}</span> : null}
          {proxy.timezone ? <span className="proxy-row__country">{proxy.timezone}</span> : null}
        </div>
      </div>

      <ProxyTestBadge state={testState} result={testResult} />

      <div className="proxy-row__actions">
        <Button
          variant="secondary"
          size="sm"
          onClick={() => onTest(proxy)}
          disabled={testState === "running"}
        >
          {t("proxy.test")}
        </Button>
        <IconButton label={t("common.edit")} size="sm" onClick={() => onEdit(proxy)}>
          <Icon name="edit" size={14} />
        </IconButton>
        {proxy.hasPassword || proxy.username ? (
          <IconButton
            label={t("proxy.credentials.reveal")}
            size="sm"
            onClick={() => onReveal(proxy)}
          >
            <Icon name="eye" size={14} />
          </IconButton>
        ) : null}
        <IconButton label={t("common.delete")} size="sm" onClick={() => onDelete(proxy)}>
          <Icon name="trash" size={14} />
        </IconButton>
      </div>
    </div>
  );
}
