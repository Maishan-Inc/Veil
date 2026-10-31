use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSettingsRecord {
    pub id: String,
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub reasoning_effort: String,
    pub review_mode: String,
    pub network_enabled: bool,
    pub api_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiModelConfigRecord {
    pub id: String,
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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UpsertAiModelConfig {
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

#[derive(Debug, Clone)]
pub struct UpsertAiSettings {
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub reasoning_effort: String,
    pub review_mode: String,
    pub network_enabled: bool,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiTaskRecord {
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

#[derive(Debug, Clone)]
pub struct CreateAiTask {
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
}

#[derive(Debug, Clone)]
pub struct UpdateAiTask {
    pub status: String,
    pub session_id: Option<String>,
    pub cdp_url: Option<String>,
    pub requires_user_action: bool,
    pub pending_approval_code: Option<String>,
    pub pending_approval_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiTaskEventRecord {
    pub id: String,
    pub task_id: String,
    pub actor: String,
    pub kind: String,
    pub action: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateAiTaskEvent {
    pub task_id: String,
    pub actor: String,
    pub kind: String,
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAuditLogRecord {
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

#[derive(Debug, Clone, Default)]
pub struct AiAuditLogFilter {
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProfileFactRecord {
    pub id: String,
    pub profile_id: String,
    pub key_name: String,
    pub value_text: String,
    pub source: String,
    pub sensitivity: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UpsertAiProfileFact {
    pub profile_id: String,
    pub key_name: String,
    pub value_text: String,
    pub source: String,
    pub sensitivity: String,
}
