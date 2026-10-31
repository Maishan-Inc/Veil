import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { ProxyTestResult } from "@veil/shared";
import { Spinner } from "@veil/ui";
import { countryFlag } from "./format";

interface ProxyTestBadgeProps {
  state: "idle" | "running" | "result";
  result: ProxyTestResult | null;
}

/** Inline status pill rendered next to a ProxyRow while/after testing. */
export function ProxyTestBadge({ state, result }: ProxyTestBadgeProps): ReactNode {
  const t = useT();

  if (state === "running") {
    return (
      <span className="proxy-test-badge proxy-test-badge--running">
        <Spinner size={12} label={t("proxy.test.running")} />
        {t("proxy.test.running")}
      </span>
    );
  }

  if (state !== "result" || !result) return null;

  if (!result.ok) {
    const code = result.code ?? "proxy.testFailed";
    return (
      <span className="proxy-test-badge proxy-test-badge--error">✗ {t(`errors.${code}`)}</span>
    );
  }

  const flag = countryFlag(result.countryCode);
  const location = [flag, result.country, result.city ?? result.region].filter(Boolean).join(" · ");

  return (
    <span className="proxy-test-badge proxy-test-badge--ok">
      ✓ {result.ip ?? "?"}
      {location ? ` · ${location}` : ""}
      {result.timezone ? ` · ${result.timezone}` : ""}
      {result.latencyMs != null ? ` · ${result.latencyMs} ms` : ""}
    </span>
  );
}
