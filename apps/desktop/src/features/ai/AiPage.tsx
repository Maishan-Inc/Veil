import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useT } from "@veil/i18n";
import { Badge, Button, Select, Spinner, TextField } from "@veil/ui";
import type {
  AiProfileFact,
  AiReviewMode,
  AiSettings,
  AiTask,
  AiTaskEvent,
  Profile,
} from "@veil/shared";
import {
  AppError,
  decideAiTaskApproval,
  getAiSettings,
  launchProfileForAi,
  listAiProfileFacts,
  listAiTaskEvents,
  listAiTasks,
  listProfiles,
  runAiTask,
  upsertAiProfileFact,
} from "../../api/client";

type FactDraft = {
  key: string;
  value: string;
  source: string;
  sensitivity: "standard" | "sensitive";
};

const EMPTY_FACT: FactDraft = {
  key: "gmail.email",
  value: "",
  source: "manual",
  sensitivity: "sensitive",
};

export function AiPage(): ReactNode {
  const t = useT();
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [settings, setSettings] = useState<AiSettings | null>(null);
  const [profileId, setProfileId] = useState("");
  const [prompt, setPrompt] = useState("");
  const [mode, setMode] = useState<AiReviewMode>("reviewed");
  const [tasks, setTasks] = useState<AiTask[]>([]);
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [events, setEvents] = useState<AiTaskEvent[]>([]);
  const [facts, setFacts] = useState<AiProfileFact[]>([]);
  const [factDraft, setFactDraft] = useState<FactDraft>(EMPTY_FACT);
  const [busy, setBusy] = useState(false);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  useEffect(() => {
    void loadInitial();
  }, []);

  useEffect(() => {
    if (!profileId) return;
    void refreshProfileData(profileId);
  }, [profileId]);

  useEffect(() => {
    if (!activeTaskId) {
      setEvents([]);
      return;
    }
    void listAiTaskEvents(activeTaskId)
      .then(setEvents)
      .catch(() => setEvents([]));
    const timer = window.setInterval(() => {
      void refreshActiveTask(activeTaskId);
    }, 2500);
    return () => window.clearInterval(timer);
  }, [activeTaskId]);

  const activeTask = useMemo(
    () => tasks.find((task) => task.id === activeTaskId) ?? tasks[0] ?? null,
    [activeTaskId, tasks],
  );

  async function loadInitial(): Promise<void> {
    try {
      const [nextProfiles, nextSettings] = await Promise.all([listProfiles(), getAiSettings()]);
      setProfiles(nextProfiles);
      setSettings(nextSettings);
      const first = nextProfiles[0]?.id ?? "";
      setProfileId(first);
      if (first) await refreshProfileData(first);
    } catch (err) {
      showError(err);
    }
  }

  async function refreshProfileData(nextProfileId = profileId): Promise<void> {
    if (!nextProfileId) return;
    try {
      const [nextTasks, nextFacts] = await Promise.all([
        listAiTasks(nextProfileId),
        listAiProfileFacts(nextProfileId),
      ]);
      setTasks(nextTasks);
      setFacts(nextFacts);
      setActiveTaskId((current) => current ?? nextTasks[0]?.id ?? null);
    } catch (err) {
      showError(err);
    }
  }

  async function refreshActiveTask(taskId: string): Promise<void> {
    try {
      const [nextTasks, nextEvents] = await Promise.all([
        profileId ? listAiTasks(profileId) : Promise.resolve(tasks),
        listAiTaskEvents(taskId),
      ]);
      setTasks(nextTasks);
      setEvents(nextEvents);
    } catch {
      // Polling is best-effort; explicit actions still surface errors.
    }
  }

  async function startTask(): Promise<void> {
    if (!profileId || prompt.trim().length === 0) return;
    setBusy(true);
    setErrorKey(null);
    try {
      const result = await launchProfileForAi({
        profileId,
        modelConfigId: null,
        prompt: prompt.trim(),
        executionMode: mode,
      });
      setPrompt("");
      setActiveTaskId(result.task.id);
      await refreshProfileData(profileId);
      setEvents(await listAiTaskEvents(result.task.id));
    } catch (err) {
      showError(err);
    } finally {
      setBusy(false);
    }
  }

  async function decide(approved: boolean): Promise<void> {
    if (!activeTask) return;
    setBusy(true);
    setErrorKey(null);
    try {
      const updated = await decideAiTaskApproval(activeTask.id, approved);
      setTasks((current) => current.map((task) => (task.id === updated.id ? updated : task)));
      setEvents(await listAiTaskEvents(updated.id));
    } catch (err) {
      showError(err);
    } finally {
      setBusy(false);
    }
  }

  async function continueTask(): Promise<void> {
    if (!activeTask) return;
    setBusy(true);
    setErrorKey(null);
    try {
      const result = await runAiTask(activeTask.id);
      setTasks((current) =>
        current.map((task) => (task.id === result.task.id ? result.task : task)),
      );
      setEvents(await listAiTaskEvents(result.task.id));
    } catch (err) {
      showError(err);
    } finally {
      setBusy(false);
    }
  }

  async function saveFact(): Promise<void> {
    if (!profileId || !factDraft.key.trim() || !factDraft.value.trim()) return;
    setBusy(true);
    setErrorKey(null);
    try {
      const fact = await upsertAiProfileFact({
        profileId,
        key: factDraft.key.trim(),
        value: factDraft.value.trim(),
        source: factDraft.source.trim() || "manual",
        sensitivity: factDraft.sensitivity,
      });
      setFacts((current) => [fact, ...current.filter((item) => item.key !== fact.key)]);
      setFactDraft(EMPTY_FACT);
    } catch (err) {
      showError(err);
    } finally {
      setBusy(false);
    }
  }

  function showError(err: unknown): void {
    const code = err instanceof AppError ? err.code : "app.unknown";
    setErrorKey(`errors.${code}`);
  }

  return (
    <section className="veil-section ai-page">
      <header className="veil-section__header">
        <div>
          <h2 className="veil-section__title">{t("ai.title")}</h2>
          <p className="ai-page__subtitle">{t("ai.subtitle")}</p>
        </div>
      </header>

      <section className="ai-grid">
        <div className="ai-panel ai-panel--primary">
          <div className="ai-panel__header">
            <h3>{t("ai.task.title")}</h3>
            <Badge tone={settings?.hasApiKey && settings.networkEnabled ? "success" : "warning"}>
              {settings?.hasApiKey && settings.networkEnabled
                ? t("ai.provider.ready")
                : t("ai.provider.needsSetup")}
            </Badge>
          </div>
          <div className="ai-form">
            <Select
              label={t("ai.field.profile")}
              value={profileId}
              onChange={(event) => setProfileId(event.currentTarget.value)}
              options={profiles.map((profile) => ({ value: profile.id, label: profile.name }))}
            />
            <Select
              label={t("ai.field.mode")}
              value={mode}
              onChange={(event) => setMode(event.currentTarget.value as AiReviewMode)}
              options={[
                { value: "reviewed", label: t("ai.mode.reviewed") },
                { value: "fullAuto", label: t("ai.mode.fullAuto") },
              ]}
            />
            <label className="ai-textarea-field">
              <span>{t("ai.field.prompt")}</span>
              <textarea
                className="veil-input ai-textarea"
                value={prompt}
                onChange={(event) => setPrompt(event.currentTarget.value)}
                placeholder={t("ai.field.prompt.placeholder")}
              />
            </label>
            <div className="ai-actions">
              <Button
                variant="primary"
                disabled={busy || !profileId || prompt.trim().length === 0}
                onClick={() => void startTask()}
                aria-label={busy ? t("common.loading") : undefined}
              >
                {busy ? <Spinner size={16} label={t("common.loading")} /> : t("ai.task.start")}
              </Button>
              <Button
                variant="secondary"
                disabled={!profileId || busy}
                onClick={() => void refreshProfileData(profileId)}
              >
                {t("toolbar.refresh")}
              </Button>
            </div>
            <p className="ai-note">{t("ai.policy.note")}</p>
            {errorKey ? <p className="settings-card__error">{t(errorKey)}</p> : null}
          </div>
        </div>

        <div className="ai-panel">
          <div className="ai-panel__header">
            <h3>{t("ai.current.title")}</h3>
            {activeTask ? (
              <Badge tone={taskTone(activeTask.status)}>
                {t(`ai.status.${activeTask.status}`)}
              </Badge>
            ) : null}
          </div>
          {activeTask ? (
            <div className="ai-task-detail">
              <p>{activeTask.prompt}</p>
              <dl className="ai-kv">
                <dt>{t("ai.current.session")}</dt>
                <dd>{activeTask.sessionId ?? t("common.none")}</dd>
                <dt>{t("ai.policy.humanVerification")}</dt>
                <dd>{t("ai.policy.humanVerification.value")}</dd>
                <dt>{t("ai.policy.payment")}</dt>
                <dd>{t("ai.policy.payment.value")}</dd>
              </dl>
              {activeTask.requiresUserAction ? (
                <div className="ai-approval">
                  <strong>{activeTask.pendingApprovalMessage ?? t("ai.approval.default")}</strong>
                  <div className="ai-actions">
                    {activeTask.status === "awaitingApproval" ? (
                      <>
                        <Button variant="primary" disabled={busy} onClick={() => void decide(true)}>
                          {t("ai.approval.approve")}
                        </Button>
                        <Button
                          variant="secondary"
                          disabled={busy}
                          onClick={() => void decide(false)}
                        >
                          {t("ai.approval.reject")}
                        </Button>
                      </>
                    ) : (
                      <Button variant="primary" disabled={busy} onClick={() => void continueTask()}>
                        {t("ai.task.continue")}
                      </Button>
                    )}
                  </div>
                </div>
              ) : null}
            </div>
          ) : (
            <p className="ai-empty">{t("ai.current.empty")}</p>
          )}
        </div>
      </section>

      <section className="ai-grid ai-grid--lower">
        <div className="ai-panel">
          <div className="ai-panel__header">
            <h3>{t("ai.facts.title")}</h3>
          </div>
          <div className="ai-fact-form">
            <TextField
              label={t("ai.facts.key")}
              value={factDraft.key}
              onChange={(event) => setFactDraft({ ...factDraft, key: event.currentTarget.value })}
            />
            <TextField
              label={t("ai.facts.value")}
              value={factDraft.value}
              onChange={(event) => setFactDraft({ ...factDraft, value: event.currentTarget.value })}
            />
            <Select
              label={t("ai.facts.sensitivity")}
              value={factDraft.sensitivity}
              onChange={(event) =>
                setFactDraft({
                  ...factDraft,
                  sensitivity: event.currentTarget.value as "standard" | "sensitive",
                })
              }
              options={[
                { value: "standard", label: t("ai.facts.standard") },
                { value: "sensitive", label: t("ai.facts.sensitive") },
              ]}
            />
            <Button
              variant="secondary"
              disabled={busy || !profileId}
              onClick={() => void saveFact()}
            >
              {t("common.save")}
            </Button>
          </div>
          <div className="ai-list">
            {facts.length === 0 ? (
              <p className="ai-empty">{t("ai.facts.empty")}</p>
            ) : (
              facts.map((fact) => (
                <div className="ai-list-row" key={fact.id}>
                  <span>{fact.key}</span>
                  <strong>{fact.value}</strong>
                  <Badge tone={fact.sensitivity === "sensitive" ? "warning" : "neutral"}>
                    {t(`ai.facts.${fact.sensitivity}`)}
                  </Badge>
                </div>
              ))
            )}
          </div>
        </div>

        <div className="ai-panel">
          <div className="ai-panel__header">
            <h3>{t("ai.events.title")}</h3>
          </div>
          <div className="ai-list">
            {events.length === 0 ? (
              <p className="ai-empty">{t("ai.events.empty")}</p>
            ) : (
              events.map((event) => (
                <div className="ai-event" key={event.id} data-kind={event.kind}>
                  <span>{new Date(event.createdAt).toLocaleTimeString()}</span>
                  <p>{event.message}</p>
                </div>
              ))
            )}
          </div>
        </div>
      </section>
    </section>
  );
}

function taskTone(status: AiTask["status"]): "neutral" | "success" | "warning" | "error" | "info" {
  if (status === "completed") return "success";
  if (status === "failed" || status === "cancelled") return "error";
  if (status === "awaitingApproval" || status === "pausedForUser") return "warning";
  if (status === "running") return "info";
  return "neutral";
}
