import type { ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Button, ProgressRing } from "@veil/ui";
import { useAppStore } from "../../app/store";
import { useEngineInstallStore } from "../../app/engineInstallStore";
import { buildInstallDockView } from "./installDock";
import { Icon } from "../../shell/Icon";

/**
 * Floating bottom-right install indicator, mounted once by `App`.
 *
 * The install dialog closes as soon as the user commits, so this is the only thing on screen
 * that reports a ~2-minute download. It therefore has to carry a real value, not just a
 * spinner: the ring shows the stage percentage where the backend reports one, and only falls
 * back to an indeterminate sweep for the stages that genuinely have no total (checksum,
 * finalize). The halo keeps breathing either way, so a slow download never reads as frozen.
 */
export function EngineInstallDock(): ReactNode {
  const t = useT();
  const job = useEngineInstallStore((state) => state.job);
  const cancel = useEngineInstallStore((state) => state.cancel);
  const dismiss = useEngineInstallStore((state) => state.dismiss);
  const start = useEngineInstallStore((state) => state.start);
  const setSection = useAppStore((state) => state.setSection);

  if (!job) return null;

  const view = buildInstallDockView(job);
  const version = job.target.candidate.version;
  const detail = t(view.detailKey, view.detailParams);
  const title =
    view.tone === "error" ? t("browser.dock.failed") : t("browser.dock.installing", { version });

  return (
    <aside className="engine-dock" data-tone={view.tone}>
      <button
        type="button"
        className="engine-dock__summary"
        onClick={() => setSection("kernel")}
        title={t("browser.dock.openPage")}
      >
        <ProgressRing
          value={view.percent ?? undefined}
          pulse={view.pulse}
          tone={view.tone === "error" ? "error" : "accent"}
          size={44}
          label={`${title} — ${detail}`}
        >
          {view.percent === null ? null : `${view.percent}%`}
        </ProgressRing>
        <span className="engine-dock__copy">
          <strong className="engine-dock__title">{title}</strong>
          <span className="engine-dock__detail">{detail}</span>
        </span>
      </button>

      <div className="engine-dock__actions">
        {view.action === "retry" ? (
          <>
            <Button size="sm" variant="secondary" onClick={() => void start(job.target)}>
              {t("browser.action.retry")}
            </Button>
            <Button size="sm" variant="subtle" onClick={dismiss}>
              <Icon name="x" size={14} />
              {t("common.close")}
            </Button>
          </>
        ) : (
          <Button size="sm" variant="subtle" onClick={() => void cancel()}>
            <Icon name="x" size={14} />
            {t("browser.action.cancel")}
          </Button>
        )}
      </div>
    </aside>
  );
}
