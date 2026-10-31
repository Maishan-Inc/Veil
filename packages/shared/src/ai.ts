export type AiProviderKind = "openai" | "openaiCompatible";
export type AiReasoningEffort = "low" | "medium" | "high";
export type AiReviewMode = "reviewed" | "fullAuto";
export type AiTaskStatus =
  "ready" | "running" | "pausedForUser" | "awaitingApproval" | "completed" | "cancelled" | "failed";
export type AiTaskEventKind = "info" | "warning" | "approval" | "error";
export type AiTaskActor = "system" | "user" | "ai";
export type AiFactSensitivity = "standard" | "sensitive";
export type AiModelIcon =
  "openai" | "claude" | "deepseek" | "gemini" | "qwen" | "grok" | "ollama" | "custom";

export interface AiSettings {
  providerKind: AiProviderKind;
  baseUrl: string;
  model: string;
  reasoningEffort: AiReasoningEffort;
  reviewMode: AiReviewMode;
  networkEnabled: boolean;
  hasApiKey: boolean;
  updatedAt: string | null;
}

export interface UpdateAiSettingsInput {
  providerKind: AiProviderKind;
  baseUrl: string;
  model: string;
  reasoningEffort: AiReasoningEffort;
  reviewMode: AiReviewMode;
  networkEnabled: boolean;
  /**
   * `null` keeps the existing key.
   * `""` clears the stored key.
   * non-empty replaces the stored key.
   */
  apiKey: string | null;
}

export interface AiModelConfig {
  id: string;
  name: string;
  providerKind: AiProviderKind;
  icon: AiModelIcon;
  baseUrl: string;
  model: string;
  reasoningEffort: AiReasoningEffort;
  reviewMode: AiReviewMode;
  networkEnabled: boolean;
  hasApiKey: boolean;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface UpsertAiModelConfigInput {
  id: string | null;
  name: string;
  providerKind: AiProviderKind;
  icon: AiModelIcon;
  baseUrl: string;
  model: string;
  reasoningEffort: AiReasoningEffort;
  reviewMode: AiReviewMode;
  networkEnabled: boolean;
  /**
   * `null` keeps the existing key when editing.
   * `""` clears the stored key.
   * non-empty replaces the stored key.
   */
  apiKey: string | null;
  isDefault: boolean;
}

export interface AiAutomationSession {
  sessionId: string;
  profileId: string;
  processId: number;
  createdAt: string;
}

export interface AiTask {
  id: string;
  profileId: string;
  modelConfigId: string | null;
  prompt: string;
  executionMode: AiReviewMode;
  status: AiTaskStatus;
  sessionId: string | null;
  /** Redacted by the backend; retained as nullable for old persisted rows/API compatibility. */
  cdpUrl: string | null;
  requiresUserAction: boolean;
  pendingApprovalCode: string | null;
  pendingApprovalMessage: string | null;
  humanVerificationPolicy: "retryOnceThenPause";
  paymentPolicy: "userRequired";
  createdAt: string;
  updatedAt: string;
}

export interface AiTaskEvent {
  id: string;
  taskId: string;
  actor: AiTaskActor;
  kind: AiTaskEventKind;
  action: string;
  message: string;
  createdAt: string;
}

export interface AiAuditLog {
  id: string;
  taskId: string;
  profileId: string;
  profileName: string;
  actor: AiTaskActor;
  kind: AiTaskEventKind;
  action: string;
  message: string;
  createdAt: string;
}

export interface AiAuditLogFilter {
  startAt: string | null;
  endAt: string | null;
  actor: AiTaskActor | "all";
}

export interface ExportAiAuditLogsResult {
  path: string;
  count: number;
}

export interface AiProfileFact {
  id: string;
  profileId: string;
  key: string;
  value: string;
  source: string;
  sensitivity: AiFactSensitivity;
  createdAt: string;
  updatedAt: string;
}

export interface StartAiTaskInput {
  profileId: string;
  modelConfigId: string | null;
  prompt: string;
  executionMode: AiReviewMode;
}

export interface StartAiTaskResult {
  task: AiTask;
  session: AiAutomationSession;
}

export interface RunAiTaskResult {
  task: AiTask;
  started: boolean;
}

export interface UpsertAiProfileFactInput {
  profileId: string;
  key: string;
  value: string;
  source: string;
  sensitivity: AiFactSensitivity;
}
