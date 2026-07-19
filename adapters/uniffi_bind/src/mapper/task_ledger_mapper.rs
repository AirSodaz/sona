use sona_core::task_ledger::types::{
    TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTaskLedgerKindV1 {
    BatchImport,
    Automation,
    LlmPolish,
    LlmTranslate,
    LlmSummary,
    Recovery,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTaskLedgerStatusV1 {
    Pending,
    Running,
    CancelRequested,
    Failed,
    Recoverable,
    Interrupted,
    Cancelled,
    Succeeded,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiStringPatchV1 {
    Unchanged,
    Clear,
    Set { value: String },
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTaskLedgerRecordV1 {
    pub id: String,
    pub kind: FfiTaskLedgerKindV1,
    pub status: FfiTaskLedgerStatusV1,
    pub title: String,
    pub progress: f64,
    pub created_at: u64,
    pub updated_at: u64,
    pub retryable: bool,
    pub cancelable: bool,
    pub recoverable: bool,
    pub stage: Option<String>,
    pub history_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub file_path: Option<String>,
    pub automation_rule_id: Option<String>,
    pub source_fingerprint: Option<String>,
    pub error_message: Option<String>,
    pub template_id: Option<String>,
    pub target_language: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTaskLedgerPatchV1 {
    pub kind: Option<FfiTaskLedgerKindV1>,
    pub status: Option<FfiTaskLedgerStatusV1>,
    pub title: Option<String>,
    pub progress: Option<f64>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
    pub retryable: Option<bool>,
    pub cancelable: Option<bool>,
    pub recoverable: Option<bool>,
    pub stage: FfiStringPatchV1,
    pub history_id: FfiStringPatchV1,
    pub tag_ids: Option<Vec<String>>,
    pub file_path: FfiStringPatchV1,
    pub automation_rule_id: FfiStringPatchV1,
    pub source_fingerprint: FfiStringPatchV1,
    pub error_message: FfiStringPatchV1,
    pub template_id: FfiStringPatchV1,
    pub target_language: FfiStringPatchV1,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTaskLedgerSnapshotV1 {
    pub version: u32,
    pub updated_at: Option<u64>,
    pub tasks: Vec<FfiTaskLedgerRecordV1>,
}

impl From<FfiTaskLedgerKindV1> for TaskLedgerKind {
    fn from(value: FfiTaskLedgerKindV1) -> Self {
        match value {
            FfiTaskLedgerKindV1::BatchImport => Self::BatchImport,
            FfiTaskLedgerKindV1::Automation => Self::Automation,
            FfiTaskLedgerKindV1::LlmPolish => Self::LlmPolish,
            FfiTaskLedgerKindV1::LlmTranslate => Self::LlmTranslate,
            FfiTaskLedgerKindV1::LlmSummary => Self::LlmSummary,
            FfiTaskLedgerKindV1::Recovery => Self::Recovery,
            FfiTaskLedgerKindV1::Update => Self::Update,
        }
    }
}

impl From<TaskLedgerKind> for FfiTaskLedgerKindV1 {
    fn from(value: TaskLedgerKind) -> Self {
        match value {
            TaskLedgerKind::BatchImport => Self::BatchImport,
            TaskLedgerKind::Automation => Self::Automation,
            TaskLedgerKind::LlmPolish => Self::LlmPolish,
            TaskLedgerKind::LlmTranslate => Self::LlmTranslate,
            TaskLedgerKind::LlmSummary => Self::LlmSummary,
            TaskLedgerKind::Recovery => Self::Recovery,
            TaskLedgerKind::Update => Self::Update,
        }
    }
}

impl From<FfiTaskLedgerStatusV1> for TaskLedgerStatus {
    fn from(value: FfiTaskLedgerStatusV1) -> Self {
        match value {
            FfiTaskLedgerStatusV1::Pending => Self::Pending,
            FfiTaskLedgerStatusV1::Running => Self::Running,
            FfiTaskLedgerStatusV1::CancelRequested => Self::CancelRequested,
            FfiTaskLedgerStatusV1::Failed => Self::Failed,
            FfiTaskLedgerStatusV1::Recoverable => Self::Recoverable,
            FfiTaskLedgerStatusV1::Interrupted => Self::Interrupted,
            FfiTaskLedgerStatusV1::Cancelled => Self::Cancelled,
            FfiTaskLedgerStatusV1::Succeeded => Self::Succeeded,
        }
    }
}

impl From<TaskLedgerStatus> for FfiTaskLedgerStatusV1 {
    fn from(value: TaskLedgerStatus) -> Self {
        match value {
            TaskLedgerStatus::Pending => Self::Pending,
            TaskLedgerStatus::Running => Self::Running,
            TaskLedgerStatus::CancelRequested => Self::CancelRequested,
            TaskLedgerStatus::Failed => Self::Failed,
            TaskLedgerStatus::Recoverable => Self::Recoverable,
            TaskLedgerStatus::Interrupted => Self::Interrupted,
            TaskLedgerStatus::Cancelled => Self::Cancelled,
            TaskLedgerStatus::Succeeded => Self::Succeeded,
        }
    }
}

impl FfiStringPatchV1 {
    pub(crate) fn into_core(self) -> Option<Option<String>> {
        match self {
            Self::Unchanged => None,
            Self::Clear => Some(None),
            Self::Set { value } => Some(Some(value)),
        }
    }
}

impl From<FfiTaskLedgerRecordV1> for TaskLedgerRecord {
    fn from(value: FfiTaskLedgerRecordV1) -> Self {
        Self {
            id: value.id,
            kind: value.kind.into(),
            status: value.status.into(),
            title: value.title,
            progress: value.progress,
            created_at: value.created_at,
            updated_at: value.updated_at,
            retryable: value.retryable,
            cancelable: value.cancelable,
            recoverable: value.recoverable,
            stage: value.stage,
            history_id: value.history_id,
            tag_ids: value.tag_ids,
            file_path: value.file_path,
            automation_rule_id: value.automation_rule_id,
            source_fingerprint: value.source_fingerprint,
            error_message: value.error_message,
            template_id: value.template_id,
            target_language: value.target_language,
        }
    }
}

impl From<TaskLedgerRecord> for FfiTaskLedgerRecordV1 {
    fn from(value: TaskLedgerRecord) -> Self {
        Self {
            id: value.id,
            kind: value.kind.into(),
            status: value.status.into(),
            title: value.title,
            progress: value.progress,
            created_at: value.created_at,
            updated_at: value.updated_at,
            retryable: value.retryable,
            cancelable: value.cancelable,
            recoverable: value.recoverable,
            stage: value.stage,
            history_id: value.history_id,
            tag_ids: value.tag_ids,
            file_path: value.file_path,
            automation_rule_id: value.automation_rule_id,
            source_fingerprint: value.source_fingerprint,
            error_message: value.error_message,
            template_id: value.template_id,
            target_language: value.target_language,
        }
    }
}

impl From<FfiTaskLedgerPatchV1> for TaskLedgerPatch {
    fn from(value: FfiTaskLedgerPatchV1) -> Self {
        Self {
            kind: value.kind.map(Into::into),
            status: value.status.map(Into::into),
            title: value.title,
            progress: value.progress,
            created_at: value.created_at,
            updated_at: value.updated_at,
            retryable: value.retryable,
            cancelable: value.cancelable,
            recoverable: value.recoverable,
            stage: value.stage.into_core(),
            history_id: value.history_id.into_core(),
            tag_ids: value.tag_ids,
            file_path: value.file_path.into_core(),
            automation_rule_id: value.automation_rule_id.into_core(),
            source_fingerprint: value.source_fingerprint.into_core(),
            error_message: value.error_message.into_core(),
            template_id: value.template_id.into_core(),
            target_language: value.target_language.into_core(),
        }
    }
}

impl From<TaskLedgerSnapshot> for FfiTaskLedgerSnapshotV1 {
    fn from(value: TaskLedgerSnapshot) -> Self {
        Self {
            version: value.version,
            updated_at: value.updated_at,
            tasks: value.tasks.into_iter().map(Into::into).collect(),
        }
    }
}
