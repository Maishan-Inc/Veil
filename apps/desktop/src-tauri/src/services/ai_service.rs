use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use veil_core::{AppError, AppResult, ErrorCode};
use veil_database::{
    AiAuditLogFilter as DbAiAuditLogFilter, AiAuditLogRecord, AiModelConfigRecord,
    AiProfileFactRecord, AiSettingsRecord, AiTaskEventRecord, AiTaskRecord, CreateAiTask,
    CreateAiTaskEvent, Database, DatabaseError, UpdateAiTask, UpsertAiModelConfig,
    UpsertAiProfileFact, UpsertAiSettings,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub reasoning_effort: String,
    pub review_mode: String,
    pub network_enabled: bool,
    pub has_api_key: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAiSettingsInput {
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub reasoning_effort: String,
    pub review_mode: String,
    pub network_enabled: bool,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelConfig {
    pub id: String,
    pub name: String,
    pub provider_kind: String,
    pub icon: String,
    pub base_url: String,
    pub model: String,
    pub reasoning_effort: String,
    pub review_mode: String,
    pub network_enabled: bool,
    pub has_api_key: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAiModelConfigInput {
    pub id: Option<String>,
    pub name: String,
    pub provider_kind: String,
    pub icon: String,
    pub base_url: String,
    pub model: String,
    pub reasoning_effort: String,
    pub review_mode: String,
    pub network_enabled: bool,
    pub api_key: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAutomationSession {
    pub session_id: String,
    pub profile_id: String,
    pub process_id: u32,
    #[serde(skip_serializing)]
    pub cdp_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTask {
    pub id: String,
    pub profile_id: String,
    pub model_config_id: Option<String>,
    pub prompt: String,
    pub execution_mode: String,
    pub status: String,
    pub session_id: Option<String>,
    pub cdp_url: Option<String>,
    pub requires_user_action: bool,
    pub pending_approval_code: Option<String>,
    pub pending_approval_message: Option<String>,
    pub human_verification_policy: String,
    pub payment_policy: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTaskEvent {
    pub id: String,
    pub task_id: String,
    pub actor: String,
    pub kind: String,
    pub action: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAuditLog {
    pub id: String,
    pub task_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub actor: String,
    pub kind: String,
    pub action: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAuditLogFilter {
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAiAuditLogsResult {
    pub path: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfileFact {
    pub id: String,
    pub profile_id: String,
    pub key: String,
    pub value: String,
    pub source: String,
    pub sensitivity: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAiTaskInput {
    pub profile_id: String,
    pub model_config_id: Option<String>,
    pub prompt: String,
    pub execution_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAiTaskResult {
    pub task: AiTask,
    pub session: AiAutomationSession,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAiProfileFactInput {
    pub profile_id: String,
    pub key: String,
    pub value: String,
    pub source: String,
    pub sensitivity: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveAiTaskInput {
    pub task_id: String,
    pub approved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAiTaskResult {
    pub task: AiTask,
    pub started: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerTaskRequest {
    kind: String,
    task_id: String,
    profile_id: String,
    prompt: String,
    execution_mode: String,
    cdp_url: String,
    provider: WorkerProvider,
    facts: Vec<WorkerFact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerProvider {
    kind: String,
    base_url: String,
    model: String,
    api_key: String,
    reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkerFact {
    key: String,
    value: String,
    source: String,
    sensitivity: String,
}

struct ProviderConfig {
    id: Option<String>,
    provider_kind: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
    reasoning_effort: String,
    network_enabled: bool,
    has_api_key: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WorkerMessage {
    Event {
        kind: String,
        message: String,
    },
    Result {
        status: String,
        message: String,
        #[serde(default)]
        pending_approval_code: Option<String>,
        #[serde(default)]
        pending_approval_message: Option<String>,
    },
}

#[derive(Clone)]
pub struct AiService {
    db: Arc<Mutex<Database>>,
    sessions: Arc<Mutex<HashMap<String, AiAutomationSession>>>,
}

impl AiService {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self {
            db,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Database> {
        self.db.lock().expect("database lock poisoned")
    }

    pub fn get_settings(&self) -> AppResult<AiSettings> {
        match self.db().get_ai_settings() {
            Ok(record) => Ok(map_ai_settings(record)),
            Err(DatabaseError::NotFound) => Ok(AiSettings {
                provider_kind: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.5".to_string(),
                reasoning_effort: "medium".to_string(),
                review_mode: "reviewed".to_string(),
                network_enabled: false,
                has_api_key: false,
                updated_at: None,
            }),
            Err(err) => Err(map_read_error(err)),
        }
    }

    pub fn update_settings(&self, input: UpdateAiSettingsInput) -> AppResult<AiSettings> {
        let current = self.db().get_ai_settings().ok();
        let provider_kind = normalize_provider_kind(&input.provider_kind);
        let base_url = normalize_ai_base_url(&input.base_url)?;
        let provider_target_changed = current.as_ref().is_some_and(|record| {
            record.provider_kind != provider_kind
                || normalize_ai_base_url(&record.base_url).ok() != Some(base_url.clone())
        });
        let api_key = match input.api_key.as_deref().map(str::trim) {
            None if !provider_target_changed => current.as_ref().and_then(|r| r.api_key.clone()),
            None => None,
            Some("") => None,
            Some(value) => Some(value.to_string()),
        };
        let record = self
            .db()
            .upsert_ai_settings(UpsertAiSettings {
                provider_kind,
                base_url,
                model: input.model.trim().to_string(),
                reasoning_effort: normalize_reasoning_effort(&input.reasoning_effort),
                review_mode: normalize_review_mode(&input.review_mode),
                network_enabled: input.network_enabled,
                api_key,
            })
            .map_err(map_write_error)?;
        Ok(map_ai_settings(record))
    }

    pub fn has_provider_credentials(&self) -> AppResult<bool> {
        let models = self.list_model_configs()?;
        if models
            .iter()
            .any(|model| model.network_enabled && model.has_api_key && !model.model.is_empty())
        {
            return Ok(true);
        }
        let settings = self.get_settings()?;
        Ok(settings.network_enabled && settings.has_api_key && !settings.model.is_empty())
    }

    pub fn has_provider_credentials_for(&self, model_config_id: Option<&str>) -> AppResult<bool> {
        let provider = self.resolve_provider(model_config_id)?;
        Ok(provider.network_enabled && provider.has_api_key && !provider.model.is_empty())
    }

    pub fn list_model_configs(&self) -> AppResult<Vec<AiModelConfig>> {
        let records = self.db().list_ai_model_configs().map_err(map_read_error)?;
        if !records.is_empty() {
            return Ok(records.into_iter().map(map_ai_model_config).collect());
        }
        let settings = match self.db().get_ai_settings() {
            Ok(settings) => settings,
            Err(DatabaseError::NotFound) => return Ok(Vec::new()),
            Err(err) => return Err(map_read_error(err)),
        };
        let record = self
            .db()
            .upsert_ai_model_config(UpsertAiModelConfig {
                id: None,
                name: legacy_model_name(&settings),
                provider_kind: settings.provider_kind,
                icon: "openai".to_string(),
                base_url: settings.base_url,
                model: settings.model,
                reasoning_effort: settings.reasoning_effort,
                review_mode: settings.review_mode,
                network_enabled: settings.network_enabled,
                api_key: settings.api_key,
                is_default: true,
            })
            .map_err(map_write_error)?;
        Ok(vec![map_ai_model_config(record)])
    }

    pub fn upsert_model_config(&self, input: UpsertAiModelConfigInput) -> AppResult<AiModelConfig> {
        let existing = input
            .id
            .as_deref()
            .and_then(|id| self.db().get_ai_model_config(id).ok());
        let provider_kind = normalize_provider_kind(&input.provider_kind);
        let base_url = normalize_ai_base_url(&input.base_url)?;
        let target_changed = existing.as_ref().is_some_and(|record| {
            record.provider_kind != provider_kind
                || normalize_ai_base_url(&record.base_url).ok() != Some(base_url.clone())
        });
        let api_key = match input.api_key.as_deref().map(str::trim) {
            None if !target_changed => existing.and_then(|record| record.api_key),
            None => None,
            Some("") => None,
            Some(value) => Some(value.to_string()),
        };
        let record = self
            .db()
            .upsert_ai_model_config(UpsertAiModelConfig {
                id: input.id,
                name: input.name.trim().to_string(),
                provider_kind,
                icon: normalize_model_icon(&input.icon).to_string(),
                base_url,
                model: input.model.trim().to_string(),
                reasoning_effort: normalize_reasoning_effort(&input.reasoning_effort),
                review_mode: normalize_review_mode(&input.review_mode),
                network_enabled: input.network_enabled,
                api_key,
                is_default: input.is_default,
            })
            .map_err(map_write_error)?;
        Ok(map_ai_model_config(record))
    }

    pub fn list_audit_logs(&self, filter: AiAuditLogFilter) -> AppResult<Vec<AiAuditLog>> {
        let actor = filter
            .actor
            .as_deref()
            .filter(|actor| *actor != "all")
            .map(normalize_actor)
            .map(str::to_string);
        let logs = self
            .db()
            .list_ai_audit_logs(DbAiAuditLogFilter {
                start_at: filter.start_at,
                end_at: filter.end_at,
                actor,
            })
            .map_err(map_read_error)?;
        Ok(logs.into_iter().map(map_ai_audit_log).collect())
    }

    pub fn export_audit_logs(
        &self,
        data_dir: &Path,
        filter: AiAuditLogFilter,
    ) -> AppResult<ExportAiAuditLogsResult> {
        let logs = self.list_audit_logs(filter)?;
        let export_dir = data_dir.join("exports");
        fs::create_dir_all(&export_dir)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        let stamp = now_rfc3339().replace([':', '.'], "-");
        let target = export_dir.join(format!("ai-audit-{stamp}.csv"));
        let tmp = target.with_extension("csv.tmp");
        let mut csv =
            String::from("created_at,actor,kind,action,profile_id,profile_name,task_id,message\n");
        for log in &logs {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                csv_escape(&log.created_at),
                csv_escape(&log.actor),
                csv_escape(&log.kind),
                csv_escape(&log.action),
                csv_escape(&log.profile_id),
                csv_escape(&log.profile_name),
                csv_escape(&log.task_id),
                csv_escape(&log.message),
            ));
        }
        fs::write(&tmp, csv).map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        fs::rename(&tmp, &target)
            .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        Ok(ExportAiAuditLogsResult {
            path: target.to_string_lossy().to_string(),
            count: logs.len(),
        })
    }

    pub fn allocate_debug_port(&self) -> AppResult<u16> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|_| AppError::from(ErrorCode::AiAutomationUnavailable))?;
        let port = listener
            .local_addr()
            .map_err(|_| AppError::from(ErrorCode::AiAutomationUnavailable))?
            .port();
        drop(listener);
        Ok(port)
    }

    pub fn create_session(
        &self,
        profile_id: String,
        process_id: u32,
        debug_port: u16,
    ) -> AiAutomationSession {
        let session = AiAutomationSession {
            session_id: uuid::Uuid::new_v4().to_string(),
            profile_id,
            process_id,
            cdp_url: format!("http://127.0.0.1:{debug_port}"),
            created_at: now_rfc3339(),
        };
        self.sessions
            .lock()
            .expect("sessions lock poisoned")
            .insert(session.session_id.clone(), session.clone());
        session
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<AiAutomationSession> {
        self.sessions
            .lock()
            .expect("sessions lock poisoned")
            .get(session_id)
            .cloned()
            .ok_or_else(|| AppError::from(ErrorCode::AiSessionNotFound))
    }

    pub fn start_task(
        &self,
        input: StartAiTaskInput,
        session: &AiAutomationSession,
    ) -> AppResult<AiTask> {
        let execution_mode = normalize_review_mode(&input.execution_mode);
        let requires_initial_approval = execution_mode != "fullAuto";
        let model_config_id = self.resolve_provider(input.model_config_id.as_deref())?.id;
        let task = self
            .db()
            .create_ai_task(CreateAiTask {
                profile_id: input.profile_id,
                model_config_id,
                prompt: input.prompt.trim().to_string(),
                execution_mode,
                status: if requires_initial_approval {
                    "awaitingApproval".to_string()
                } else {
                    "ready".to_string()
                },
                session_id: Some(session.session_id.clone()),
                cdp_url: None,
                requires_user_action: requires_initial_approval,
                pending_approval_code: requires_initial_approval.then(|| "task.start".to_string()),
                pending_approval_message: requires_initial_approval.then(|| {
                    "Review this task before the browser begins typing, submitting, or reading verification data."
                        .to_string()
                }),
                human_verification_policy: "retryOnceThenPause".to_string(),
                payment_policy: "userRequired".to_string(),
            })
            .map_err(map_write_error)?;
        let _ = if requires_initial_approval {
            self.append_event_as(
                &task.id,
                "user",
                "approval",
                "task.created",
                "Task created. Waiting for user approval before browser automation starts.",
            )
        } else {
            self.append_event_as(
                &task.id,
                "user",
                "info",
                "task.created",
                "Task created in full-auto mode. Automation is ready to start.",
            )
        };
        Ok(map_ai_task(task))
    }

    pub fn list_tasks_for_profile(&self, profile_id: &str) -> AppResult<Vec<AiTask>> {
        let tasks = self
            .db()
            .list_ai_tasks_for_profile(profile_id)
            .map_err(map_read_error)?;
        Ok(tasks.into_iter().map(map_ai_task).collect())
    }

    pub fn get_task(&self, task_id: &str) -> AppResult<AiTask> {
        let task = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        Ok(map_ai_task(task))
    }

    pub fn list_task_events(&self, task_id: &str) -> AppResult<Vec<AiTaskEvent>> {
        let events = self
            .db()
            .list_ai_task_events(task_id)
            .map_err(map_read_error)?;
        Ok(events.into_iter().map(map_ai_task_event).collect())
    }

    pub fn decide_task_approval(&self, input: ApproveAiTaskInput) -> AppResult<AiTask> {
        let status = if input.approved { "ready" } else { "cancelled" };
        let message = if input.approved {
            "User approved this task. Execution may continue."
        } else {
            "User rejected this task."
        };
        let previous = self
            .db()
            .get_ai_task(&input.task_id)
            .map_err(map_read_error)?;
        let task = self
            .db()
            .update_ai_task(
                &input.task_id,
                UpdateAiTask {
                    status: status.to_string(),
                    session_id: previous.session_id,
                    cdp_url: None,
                    requires_user_action: false,
                    pending_approval_code: None,
                    pending_approval_message: None,
                },
            )
            .map_err(map_write_error)?;
        let _ = self.append_event_as(&task.id, "user", "info", "task.approval", message);
        Ok(map_ai_task(task))
    }

    pub fn run_ready_task(
        &self,
        task_id: &str,
        worker_path: Option<PathBuf>,
    ) -> AppResult<RunAiTaskResult> {
        let task = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        if task.status != "ready" && task.status != "pausedForUser" {
            return Err(AppError::from(ErrorCode::AiApprovalRequired));
        }
        let request = self.build_worker_request(&task)?;
        let updated = self
            .db()
            .claim_ai_task_for_run(task_id)
            .map_err(map_write_error)?;
        let Some(updated) = updated else {
            return Err(AppError::from(ErrorCode::AiApprovalRequired));
        };
        let _ = self.append_event_as(
            task_id,
            "system",
            "info",
            "worker.starting",
            "Automation worker is starting.",
        );

        let service = self.clone();
        let task_id = task_id.to_string();
        std::thread::spawn(move || {
            if let Err(err) = service.run_worker_process(&task_id, request, worker_path) {
                let _ = service.fail_task(
                    &task_id,
                    "AI automation worker could not complete the task.",
                );
                let _ = service.append_event(
                    &task_id,
                    "error",
                    &format!("Automation worker failed: {err}"),
                );
            }
        });

        Ok(RunAiTaskResult {
            task: map_ai_task(updated),
            started: true,
        })
    }

    pub fn run_ready_task_if_needed(
        &self,
        task_id: &str,
        worker_path: Option<PathBuf>,
    ) -> AppResult<()> {
        let task = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        if task.status == "ready" {
            let _ = self.run_ready_task(task_id, worker_path)?;
        }
        Ok(())
    }

    pub fn pause_for_human_verification(&self, task_id: &str) -> AppResult<AiTask> {
        let previous = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        let task = self
            .db()
            .update_ai_task(
                task_id,
                UpdateAiTask {
                    status: "pausedForUser".to_string(),
                    session_id: previous.session_id,
                    cdp_url: None,
                    requires_user_action: true,
                    pending_approval_code: Some("human.verification".to_string()),
                    pending_approval_message: Some(
                        "Human verification could not be solved automatically. Please continue in the browser."
                            .to_string(),
                    ),
                },
            )
            .map_err(map_write_error)?;
        let _ = self.append_event_as(
            task_id,
            "system",
            "warning",
            "human_verification.pause",
            "Automation paused after a failed human-verification attempt.",
        );
        Ok(map_ai_task(task))
    }

    pub fn pause_for_user(
        &self,
        task_id: &str,
        code: Option<String>,
        message: Option<String>,
    ) -> AppResult<AiTask> {
        let previous = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        let message = message
            .unwrap_or_else(|| "The AI task needs user action before it can continue.".to_string());
        let code = code.unwrap_or_else(|| "user.required".to_string());
        let task = self
            .db()
            .update_ai_task(
                task_id,
                UpdateAiTask {
                    status: "pausedForUser".to_string(),
                    session_id: previous.session_id,
                    cdp_url: None,
                    requires_user_action: true,
                    pending_approval_code: Some(code),
                    pending_approval_message: Some(message.clone()),
                },
            )
            .map_err(map_write_error)?;
        let _ = self.append_event_as(task_id, "system", "warning", "task.paused", &message);
        Ok(map_ai_task(task))
    }

    pub fn complete_task(&self, task_id: &str, message: &str) -> AppResult<AiTask> {
        let previous = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        let task = self
            .db()
            .update_ai_task(
                task_id,
                UpdateAiTask {
                    status: "completed".to_string(),
                    session_id: previous.session_id,
                    cdp_url: None,
                    requires_user_action: false,
                    pending_approval_code: None,
                    pending_approval_message: None,
                },
            )
            .map_err(map_write_error)?;
        let _ = self.append_event_as(task_id, "ai", "info", "task.completed", message);
        Ok(map_ai_task(task))
    }

    pub fn fail_task(&self, task_id: &str, message: &str) -> AppResult<AiTask> {
        let previous = self.db().get_ai_task(task_id).map_err(map_read_error)?;
        let task = self
            .db()
            .update_ai_task(
                task_id,
                UpdateAiTask {
                    status: "failed".to_string(),
                    session_id: previous.session_id,
                    cdp_url: None,
                    requires_user_action: false,
                    pending_approval_code: None,
                    pending_approval_message: None,
                },
            )
            .map_err(map_write_error)?;
        let _ = self.append_event_as(task_id, "system", "error", "task.failed", message);
        Ok(map_ai_task(task))
    }

    pub fn append_event(&self, task_id: &str, kind: &str, message: &str) -> AppResult<AiTaskEvent> {
        self.append_event_as(task_id, "system", kind, "event", message)
    }

    pub fn append_event_as(
        &self,
        task_id: &str,
        actor: &str,
        kind: &str,
        action: &str,
        message: &str,
    ) -> AppResult<AiTaskEvent> {
        let event = self
            .db()
            .create_ai_task_event(CreateAiTaskEvent {
                task_id: task_id.to_string(),
                actor: normalize_actor(actor).to_string(),
                kind: kind.to_string(),
                action: normalize_action(action).to_string(),
                message: message.to_string(),
            })
            .map_err(map_write_error)?;
        Ok(map_ai_task_event(event))
    }

    pub fn list_profile_facts(&self, profile_id: &str) -> AppResult<Vec<AiProfileFact>> {
        let facts = self
            .db()
            .list_ai_profile_facts(profile_id)
            .map_err(map_read_error)?;
        Ok(facts.into_iter().map(map_ai_profile_fact).collect())
    }

    pub fn upsert_profile_fact(&self, input: UpsertAiProfileFactInput) -> AppResult<AiProfileFact> {
        let fact = self
            .db()
            .upsert_ai_profile_fact(UpsertAiProfileFact {
                profile_id: input.profile_id,
                key_name: input.key.trim().to_string(),
                value_text: input.value.trim().to_string(),
                source: input.source.trim().to_string(),
                sensitivity: normalize_fact_sensitivity(&input.sensitivity),
            })
            .map_err(map_write_error)?;
        Ok(map_ai_profile_fact(fact))
    }

    fn build_worker_request(&self, task: &AiTaskRecord) -> AppResult<WorkerTaskRequest> {
        let provider = self.resolve_provider(task.model_config_id.as_deref())?;
        let api_key = provider
            .api_key
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::from(ErrorCode::AiProviderMissing))?;
        if !provider.network_enabled {
            return Err(AppError::from(ErrorCode::AiProviderMissing));
        }
        let session_id = task
            .session_id
            .as_deref()
            .ok_or_else(|| AppError::from(ErrorCode::AiSessionNotFound))?;
        let session = self.get_session(session_id)?;
        if session.profile_id != task.profile_id {
            return Err(AppError::from(ErrorCode::AiSessionNotFound));
        }
        let facts = self
            .db()
            .list_ai_profile_facts(&task.profile_id)
            .map_err(map_read_error)?
            .into_iter()
            .filter(is_safe_worker_fact)
            .map(|fact| WorkerFact {
                key: fact.key_name,
                value: fact.value_text,
                source: fact.source,
                sensitivity: fact.sensitivity,
            })
            .collect();
        Ok(WorkerTaskRequest {
            kind: "runTask".to_string(),
            task_id: task.id.clone(),
            profile_id: task.profile_id.clone(),
            prompt: task.prompt.clone(),
            execution_mode: task.execution_mode.clone(),
            cdp_url: session.cdp_url,
            provider: WorkerProvider {
                kind: provider.provider_kind,
                base_url: provider.base_url,
                model: provider.model,
                api_key,
                reasoning_effort: provider.reasoning_effort,
            },
            facts,
        })
    }

    fn resolve_provider(&self, model_config_id: Option<&str>) -> AppResult<ProviderConfig> {
        if let Some(id) = model_config_id.filter(|id| !id.trim().is_empty()) {
            let record = self.db().get_ai_model_config(id).map_err(map_read_error)?;
            return Ok(provider_from_model_config(record));
        }
        let default_config = self.db().get_default_ai_model_config();
        match default_config {
            Ok(record) => Ok(provider_from_model_config(record)),
            Err(DatabaseError::NotFound) => {
                let settings = self.db().get_ai_settings().map_err(map_read_error)?;
                Ok(provider_from_settings(settings))
            }
            Err(err) => Err(map_read_error(err)),
        }
    }

    fn run_worker_process(
        &self,
        task_id: &str,
        request: WorkerTaskRequest,
        worker_path: Option<PathBuf>,
    ) -> Result<(), String> {
        let worker_path = resolve_worker_path(worker_path)?;
        let mut child = Command::new("node")
            .arg(&worker_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|err| err.to_string())?;

        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "worker stdin unavailable".to_string())?;
            let payload = serde_json::to_vec(&request).map_err(|err| err.to_string())?;
            stdin.write_all(&payload).map_err(|err| err.to_string())?;
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "worker stdout unavailable".to_string())?;
        let reader = std::io::BufReader::new(stdout);
        let mut saw_result = false;
        for line in reader.lines() {
            let line = line.map_err(|err| err.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<WorkerMessage>(&line) {
                Ok(WorkerMessage::Event { kind, message }) => {
                    let _ = self.append_event(task_id, normalize_event_kind(&kind), &message);
                }
                Ok(WorkerMessage::Result {
                    status,
                    message,
                    pending_approval_code,
                    pending_approval_message,
                }) => {
                    saw_result = true;
                    match status.as_str() {
                        "completed" => {
                            let _ = self.complete_task(task_id, &message);
                        }
                        "pausedForUser" => {
                            let _ = self.pause_for_user(
                                task_id,
                                pending_approval_code,
                                pending_approval_message.or(Some(message)),
                            );
                        }
                        _ => {
                            let _ = self.fail_task(task_id, &message);
                        }
                    }
                }
                Err(_) => {
                    let _ = self.append_event(
                        task_id,
                        "warning",
                        "Automation worker returned an unreadable message.",
                    );
                }
            }
        }

        let output = child.wait_with_output().map_err(|err| err.to_string())?;
        if !output.stderr.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = self.append_event(
                task_id,
                "warning",
                &format!("Automation worker stderr: {}", truncate_for_event(&stderr)),
            );
        }
        if !saw_result {
            return Err("worker exited without a result".to_string());
        }
        if !output.status.success() {
            return Err(format!("worker exited with status {}", output.status));
        }
        Ok(())
    }
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(not(windows))]
trait CommandCreationFlagsNoop {
    fn creation_flags(&mut self, _flags: u32) -> &mut Self;
}

#[cfg(not(windows))]
impl CommandCreationFlagsNoop for Command {
    fn creation_flags(&mut self, _flags: u32) -> &mut Self {
        self
    }
}

fn resolve_worker_path(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        #[cfg(not(debug_assertions))]
        {
            let _ = path;
            return Err(
                "custom automation worker paths are only allowed in debug builds".to_string(),
            );
        }
        #[cfg(debug_assertions)]
        {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let mut candidates = Vec::new();
    if let Some(dir) = exe_dir {
        candidates.push(dir.join("automation-worker").join("index.js"));
        candidates.push(
            dir.join("resources")
                .join("automation-worker")
                .join("index.js"),
        );
    }

    #[cfg(debug_assertions)]
    {
        if let Ok(path) = std::env::var("VEIL_AUTOMATION_WORKER") {
            candidates.push(PathBuf::from(path));
        }
        let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
        candidates.push(
            cwd.join("apps")
                .join("automation-worker")
                .join("dist")
                .join("index.js"),
        );
        candidates.push(
            cwd.join("apps")
                .join("automation-worker")
                .join("src")
                .join("index.js"),
        );
        if let Some(candidate) = ancestor_candidate(&cwd) {
            candidates.push(candidate);
        }
    }
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("automation worker script was not found".to_string())
}

fn ancestor_candidate(cwd: &Path) -> Option<PathBuf> {
    for ancestor in cwd.ancestors() {
        let candidate = ancestor
            .join("apps")
            .join("automation-worker")
            .join("dist")
            .join("index.js");
        if candidate.exists() {
            return Some(candidate);
        }
        let source_candidate = ancestor
            .join("apps")
            .join("automation-worker")
            .join("src")
            .join("index.js");
        if source_candidate.exists() {
            return Some(source_candidate);
        }
    }
    None
}

fn normalize_event_kind(kind: &str) -> &'static str {
    match kind {
        "warning" => "warning",
        "approval" => "approval",
        "error" => "error",
        _ => "info",
    }
}

fn truncate_for_event(value: &str) -> String {
    const MAX: usize = 900;
    if value.chars().count() <= MAX {
        return value.to_string();
    }
    format!("{}...", value.chars().take(MAX).collect::<String>())
}

fn map_ai_settings(record: AiSettingsRecord) -> AiSettings {
    AiSettings {
        provider_kind: record.provider_kind,
        base_url: record.base_url,
        model: record.model,
        reasoning_effort: record.reasoning_effort,
        review_mode: record.review_mode,
        network_enabled: record.network_enabled,
        has_api_key: record
            .api_key
            .as_deref()
            .is_some_and(|value| !value.is_empty()),
        updated_at: Some(record.updated_at),
    }
}

fn map_ai_model_config(record: AiModelConfigRecord) -> AiModelConfig {
    AiModelConfig {
        id: record.id,
        name: record.name,
        provider_kind: record.provider_kind,
        icon: record.icon,
        base_url: record.base_url,
        model: record.model,
        reasoning_effort: record.reasoning_effort,
        review_mode: record.review_mode,
        network_enabled: record.network_enabled,
        has_api_key: record
            .api_key
            .as_deref()
            .is_some_and(|value| !value.is_empty()),
        is_default: record.is_default,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn map_ai_task(record: AiTaskRecord) -> AiTask {
    AiTask {
        id: record.id,
        profile_id: record.profile_id,
        model_config_id: record.model_config_id,
        prompt: record.prompt,
        execution_mode: record.execution_mode,
        status: record.status,
        session_id: record.session_id,
        cdp_url: None,
        requires_user_action: record.requires_user_action,
        pending_approval_code: record.pending_approval_code,
        pending_approval_message: record.pending_approval_message,
        human_verification_policy: record.human_verification_policy,
        payment_policy: record.payment_policy,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn map_ai_task_event(record: AiTaskEventRecord) -> AiTaskEvent {
    AiTaskEvent {
        id: record.id,
        task_id: record.task_id,
        actor: record.actor,
        kind: record.kind,
        action: record.action,
        message: record.message,
        created_at: record.created_at,
    }
}

fn map_ai_audit_log(record: AiAuditLogRecord) -> AiAuditLog {
    AiAuditLog {
        id: record.id,
        task_id: record.task_id,
        profile_id: record.profile_id,
        profile_name: record.profile_name,
        actor: record.actor,
        kind: record.kind,
        action: record.action,
        message: record.message,
        created_at: record.created_at,
    }
}

fn map_ai_profile_fact(record: AiProfileFactRecord) -> AiProfileFact {
    AiProfileFact {
        id: record.id,
        profile_id: record.profile_id,
        key: record.key_name,
        value: record.value_text,
        source: record.source,
        sensitivity: record.sensitivity,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn provider_from_model_config(record: AiModelConfigRecord) -> ProviderConfig {
    let has_api_key = record
        .api_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    ProviderConfig {
        id: Some(record.id),
        provider_kind: record.provider_kind,
        base_url: record.base_url,
        model: record.model,
        api_key: record.api_key,
        reasoning_effort: record.reasoning_effort,
        network_enabled: record.network_enabled,
        has_api_key,
    }
}

fn provider_from_settings(record: AiSettingsRecord) -> ProviderConfig {
    let has_api_key = record
        .api_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    ProviderConfig {
        id: None,
        provider_kind: record.provider_kind,
        base_url: record.base_url,
        model: record.model,
        api_key: record.api_key,
        reasoning_effort: record.reasoning_effort,
        network_enabled: record.network_enabled,
        has_api_key,
    }
}

fn legacy_model_name(settings: &AiSettingsRecord) -> String {
    let model = settings.model.trim();
    if model.is_empty() {
        "Default model".to_string()
    } else {
        model.to_string()
    }
}

fn map_read_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::AiTaskNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseReadFailed.into()
        }
    }
}

fn map_write_error(err: DatabaseError) -> AppError {
    match err {
        DatabaseError::NotFound => ErrorCode::AiTaskNotFound.into(),
        DatabaseError::Sqlite(_) | DatabaseError::Serde(_) => {
            ErrorCode::StorageDatabaseWriteFailed.into()
        }
    }
}

fn normalize_provider_kind(value: &str) -> String {
    match value {
        "openaiCompatible" => "openaiCompatible".to_string(),
        _ => "openai".to_string(),
    }
}

fn normalize_model_icon(value: &str) -> &'static str {
    match value {
        "claude" => "claude",
        "deepseek" => "deepseek",
        "gemini" => "gemini",
        "qwen" => "qwen",
        "grok" => "grok",
        "ollama" => "ollama",
        "custom" => "custom",
        _ => "openai",
    }
}

fn normalize_ai_base_url(value: &str) -> AppResult<String> {
    let trimmed = value.trim().trim_end_matches('/').to_string();
    let lower = trimmed.to_ascii_lowercase();
    let is_https = lower.starts_with("https://");
    let is_local_http = lower.starts_with("http://127.0.0.1:")
        || lower == "http://127.0.0.1"
        || lower.starts_with("http://localhost:")
        || lower == "http://localhost"
        || lower.starts_with("http://[::1]:")
        || lower == "http://[::1]";
    if trimmed.is_empty() || !(is_https || is_local_http) {
        return Err(AppError::from(ErrorCode::AiProviderMissing));
    }
    Ok(trimmed)
}

fn is_safe_worker_fact(fact: &AiProfileFactRecord) -> bool {
    if fact.sensitivity != "standard" {
        return false;
    }
    let key = fact.key_name.to_ascii_lowercase();
    ![
        "pass", "password", "pwd", "token", "secret", "key", "cookie", "otp", "2fa",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn normalize_reasoning_effort(value: &str) -> String {
    match value {
        "low" | "high" => value.to_string(),
        _ => "medium".to_string(),
    }
}

fn normalize_review_mode(value: &str) -> String {
    match value {
        "fullAuto" => "fullAuto".to_string(),
        _ => "reviewed".to_string(),
    }
}

fn normalize_fact_sensitivity(value: &str) -> String {
    match value {
        "sensitive" => "sensitive".to_string(),
        _ => "standard".to_string(),
    }
}

fn normalize_actor(value: &str) -> &'static str {
    match value {
        "user" => "user",
        "ai" => "ai",
        _ => "system",
    }
}

fn normalize_action(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "event".to_string()
    } else {
        trimmed.to_string()
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use veil_core::Fingerprint;
    use veil_database::CreateProfile;

    fn service() -> AiService {
        let db = Database::open_in_memory("test-key").unwrap();
        AiService::new(Arc::new(Mutex::new(db)))
    }

    #[test]
    fn settings_round_trip_and_fact_upsert() {
        let svc = service();
        {
            let db = svc.db();
            let _ = db
                .create_profile(CreateProfile {
                    name: "Profile".to_string(),
                    avatar_color: "#5640C9".to_string(),
                    group_id: None,
                    tag_ids: vec![],
                    proxy_id: None,
                    os: "windows".to_string(),
                    platform: "Test".to_string(),
                    browser_version: "120".to_string(),
                    notes: String::new(),
                    startup_page: "veil".to_string(),
                    startup_url: String::new(),
                    fingerprint: Fingerprint::default(),
                })
                .unwrap();
        }
        let settings = svc
            .update_settings(UpdateAiSettingsInput {
                provider_kind: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-5.5".to_string(),
                reasoning_effort: "medium".to_string(),
                review_mode: "reviewed".to_string(),
                network_enabled: true,
                api_key: Some("secret".to_string()),
            })
            .unwrap();
        assert!(settings.has_api_key);

        let profile_id = svc.db().list_profiles().unwrap()[0].id.clone();
        let fact = svc
            .upsert_profile_fact(UpsertAiProfileFactInput {
                profile_id,
                key: "gmail.email".to_string(),
                value: "a@gmail.com".to_string(),
                source: "manual".to_string(),
                sensitivity: "sensitive".to_string(),
            })
            .unwrap();
        assert_eq!(fact.key, "gmail.email");
    }

    #[test]
    fn worker_request_uses_memory_session_and_excludes_sensitive_facts() {
        let svc = service();
        let profile = {
            let db = svc.db();
            db.create_profile(CreateProfile {
                name: "Profile".to_string(),
                avatar_color: "#5640C9".to_string(),
                group_id: None,
                tag_ids: vec![],
                proxy_id: None,
                os: "windows".to_string(),
                platform: "Test".to_string(),
                browser_version: "120".to_string(),
                notes: String::new(),
                startup_page: "veil".to_string(),
                startup_url: String::new(),
                fingerprint: Fingerprint::default(),
            })
            .unwrap()
        };
        svc.update_settings(UpdateAiSettingsInput {
            provider_kind: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-5.5".to_string(),
            reasoning_effort: "medium".to_string(),
            review_mode: "fullAuto".to_string(),
            network_enabled: true,
            api_key: Some("secret".to_string()),
        })
        .unwrap();
        let session = svc.create_session(profile.id.clone(), 42, 49123);
        let task = svc
            .start_task(
                StartAiTaskInput {
                    profile_id: profile.id.clone(),
                    model_config_id: None,
                    prompt: "Use saved facts".to_string(),
                    execution_mode: "fullAuto".to_string(),
                },
                &session,
            )
            .unwrap();
        assert!(task.cdp_url.is_none(), "CDP URL must not cross IPC");

        for (key, value, sensitivity) in [
            ("gmail.email", "user@example.com", "standard"),
            ("recovery_code", "123456", "sensitive"),
            ("api_token_hint", "token-value", "standard"),
        ] {
            svc.upsert_profile_fact(UpsertAiProfileFactInput {
                profile_id: profile.id.clone(),
                key: key.to_string(),
                value: value.to_string(),
                source: "manual".to_string(),
                sensitivity: sensitivity.to_string(),
            })
            .unwrap();
        }

        let record = svc.db().get_ai_task(&task.id).unwrap();
        assert!(record.cdp_url.is_none(), "CDP URL must not be persisted");
        let request = svc.build_worker_request(&record).unwrap();

        assert_eq!(request.cdp_url, session.cdp_url);
        assert_eq!(request.facts.len(), 1);
        assert_eq!(request.facts[0].key, "gmail.email");
    }
}
