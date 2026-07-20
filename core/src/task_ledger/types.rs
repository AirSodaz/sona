use serde::{Deserialize, Serialize};

pub const TASK_LEDGER_VERSION: u32 = 3;
pub const TASK_LEDGER_DIR_NAME: &str = "task-ledger";
pub const TASK_LEDGER_FILE_NAME: &str = "tasks.json";
pub const TASK_LEDGER_UPDATED_EVENT: &str = "task-ledger-updated";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TaskLedgerRecord {
    pub id: String,
    pub kind: TaskLedgerKind,
    pub status: TaskLedgerStatus,
    pub title: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub progress: f64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: u64,
    pub retryable: bool,
    pub cancelable: bool,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_automation_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_profile_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_language: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct TaskLedgerPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<TaskLedgerKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskLedgerStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub created_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub updated_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverable: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub stage: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub history_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_ids: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub file_path: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub automation_rule_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub tag_automation_rule_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub automation_profile_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub automation_profile_source: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub source_fingerprint: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub error_message: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub template_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub target_language: Option<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TaskLedgerSnapshot {
    pub version: u32,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub updated_at: Option<u64>,
    pub tasks: Vec<TaskLedgerRecord>,
}
