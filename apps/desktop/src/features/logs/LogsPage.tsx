import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import type { AiAuditLog, AiTaskActor, AiTaskEventKind } from "@veil/shared";
import { Badge, Button, Select } from "@veil/ui";
import { AppError, listAiAuditLogs } from "../../api/client";
import { formatLastLaunched } from "../profiles/format";

type ActorFilter = AiTaskActor | "all";
type KindFilter = AiTaskEventKind | "all";

const ACTOR_FILTERS: ActorFilter[] = ["all", "user", "ai", "system"];
const KIND_FILTERS: KindFilter[] = ["all", "info", "warning", "approval", "error"];

function actorTone(actor: AiTaskActor): "neutral" | "info" | "warning" {
  if (actor === "user") return "info";
  if (actor === "ai") return "warning";
  return "neutral";
}

function kindTone(kind: AiTaskEventKind): "neutral" | "info" | "warning" | "error" {
  if (kind === "error") return "error";
  if (kind === "warning" || kind === "approval") return "warning";
  return "info";
}

/**
 * Logs section: the automation activity trail, read from the existing task-event store.
 * View-only — the backend owns the data and caps the listing at its most recent entries.
 */
export function LogsPage(): ReactNode {
  const t = useT();
  const [entries, setEntries] = useState<AiAuditLog[]>([]);
  const [loading, setLoading] = useState(true);
  const [actorFilter, setActorFilter] = useState<ActorFilter>("all");
  const [kindFilter, setKindFilter] = useState<KindFilter>("all");
  const [errorKey, setErrorKey] = useState<string | null>(null);

  async function reload(): Promise<void> {
    setLoading(true);
    try {
      const logs = await listAiAuditLogs({ startAt: null, endAt: null, actor: actorFilter });
      setEntries(logs);
      setErrorKey(null);
    } catch (err) {
      const code = err instanceof AppError ? err.code : "app.unknown";
      setErrorKey(`errors.${code}`);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [actorFilter]);

  const filtered = useMemo(
    () => (kindFilter === "all" ? entries : entries.filter((entry) => entry.kind === kindFilter)),
    [entries, kindFilter],
  );

  return (
    <section className="veil-section logs-page">
      <header className="veil-section__header">
        <div>
          <h2 className="veil-section__title">{t("logs.title")}</h2>
          <p className="logs-page__subtitle">{t("logs.subtitle")}</p>
        </div>
      </header>

      <div className="logs-page__toolbar">
        <Select
          label={t("logs.filter.actor")}
          value={actorFilter}
          onChange={(event) => setActorFilter(event.currentTarget.value as ActorFilter)}
          options={ACTOR_FILTERS.map((actor) => ({
            value: actor,
            label: actor === "all" ? t("common.all") : t(`logs.actor.${actor}`),
          }))}
        />
        <Select
          label={t("logs.filter.kind")}
          value={kindFilter}
          onChange={(event) => setKindFilter(event.currentTarget.value as KindFilter)}
          options={KIND_FILTERS.map((kind) => ({
            value: kind,
            label: kind === "all" ? t("common.all") : t(`logs.kind.${kind}`),
          }))}
        />
        <span className="veil-section__spacer" />
        <span className="logs-page__count">{t("logs.entryCount", { count: filtered.length })}</span>
        <Button variant="secondary" size="sm" onClick={() => void reload()}>
          {t("toolbar.refresh")}
        </Button>
      </div>

      {errorKey ? <p className="settings-card__error">{t(errorKey)}</p> : null}

      {!loading && filtered.length === 0 ? (
        <div className="veil-empty">
          <span className="veil-empty__title">{t("logs.empty.title")}</span>
          <span>{t("logs.empty.body")}</span>
        </div>
      ) : (
        <div className="logs-list" role="list">
          {filtered.map((entry) => (
            <div className="logs-list__row" role="listitem" key={entry.id} data-kind={entry.kind}>
              <span className="logs-list__time">{formatLastLaunched(entry.createdAt, "—")}</span>
              <Badge tone={actorTone(entry.actor)}>{t(`logs.actor.${entry.actor}`)}</Badge>
              <Badge tone={kindTone(entry.kind)}>{t(`logs.kind.${entry.kind}`)}</Badge>
              <span className="logs-list__profile" title={entry.profileName}>
                {entry.profileName || "—"}
              </span>
              <span className="logs-list__message" title={entry.message}>
                {entry.message}
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
