use serde::{Deserialize, Serialize};

pub const TASK_LEDGER_VERSION: u32 = 1;
pub const TASK_LEDGER_DIR_NAME: &str = "task-ledger";
pub const TASK_LEDGER_FILE_NAME: &str = "tasks.json";
pub const TASK_LEDGER_UPDATED_EVENT: &str = "task-ledger-updated";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskLedgerKind {
    BatchImport,
    Automation,
    LlmPolish,
    LlmTranslate,
    LlmSummary,
    Recovery,
    Update,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskLedgerStatus {
    Pending,
    Running,
    CancelRequested,
    Failed,
    Recoverable,
    Interrupted,
    Cancelled,
    Succeeded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLedgerRecord {
    pub id: String,
    pub kind: TaskLedgerKind,
    pub status: TaskLedgerStatus,
    pub title: String,
    pub progress: f64,
    pub created_at: u64,
    pub updated_at: u64,
    pub retryable: bool,
    pub cancelable: bool,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_language: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLedgerSnapshot {
    pub version: u32,
    pub updated_at: Option<u64>,
    pub tasks: Vec<TaskLedgerRecord>,
}
