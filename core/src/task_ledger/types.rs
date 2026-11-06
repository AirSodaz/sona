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

impl TaskLedgerStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running | Self::CancelRequested)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Succeeded | Self::Failed | Self::Interrupted
        )
    }

    pub fn can_transition_to(&self, target: &Self) -> bool {
        if self == target {
            return true;
        }

        match self {
            Self::Cancelled | Self::Succeeded => false,
            Self::Failed | Self::Interrupted => {
                matches!(target, Self::Recoverable)
            }
            Self::Recoverable => {
                matches!(target, Self::Failed | Self::Running)
            }
            Self::Pending => {
                matches!(
                    target,
                    Self::Running
                        | Self::CancelRequested
                        | Self::Cancelled
                        | Self::Failed
                        | Self::Interrupted
                )
            }
            Self::Running => {
                matches!(
                    target,
                    Self::CancelRequested
                        | Self::Succeeded
                        | Self::Cancelled
                        | Self::Failed
                        | Self::Recoverable
                        | Self::Interrupted
                )
            }
            Self::CancelRequested => {
                matches!(target, Self::Cancelled | Self::Failed)
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_are_immutable_sinks() {
        let terminal = [TaskLedgerStatus::Cancelled, TaskLedgerStatus::Succeeded];
        let all = [
            TaskLedgerStatus::Pending,
            TaskLedgerStatus::Running,
            TaskLedgerStatus::CancelRequested,
            TaskLedgerStatus::Failed,
            TaskLedgerStatus::Recoverable,
            TaskLedgerStatus::Interrupted,
            TaskLedgerStatus::Cancelled,
            TaskLedgerStatus::Succeeded,
        ];

        for t in &terminal {
            assert!(t.is_terminal());
            assert!(!t.is_active());
            for target in &all {
                if target == t {
                    assert!(
                        t.can_transition_to(target),
                        "terminal state must be idempotent"
                    );
                } else {
                    assert!(
                        !t.can_transition_to(target),
                        "terminal state cannot transition to {:?}",
                        target
                    );
                }
            }
        }
    }

    #[test]
    fn cancel_requested_transitions() {
        let cr = TaskLedgerStatus::CancelRequested;
        assert!(cr.can_transition_to(&TaskLedgerStatus::CancelRequested));
        assert!(cr.can_transition_to(&TaskLedgerStatus::Cancelled));
        assert!(cr.can_transition_to(&TaskLedgerStatus::Failed));
        assert!(!cr.can_transition_to(&TaskLedgerStatus::Running));
        assert!(!cr.can_transition_to(&TaskLedgerStatus::Pending));
        assert!(!cr.can_transition_to(&TaskLedgerStatus::Succeeded));
    }

    #[test]
    fn running_and_pending_transitions() {
        let p = TaskLedgerStatus::Pending;
        assert!(p.can_transition_to(&TaskLedgerStatus::Running));
        assert!(p.can_transition_to(&TaskLedgerStatus::Cancelled));
        assert!(p.can_transition_to(&TaskLedgerStatus::Failed));

        let r = TaskLedgerStatus::Running;
        assert!(r.can_transition_to(&TaskLedgerStatus::Succeeded));
        assert!(r.can_transition_to(&TaskLedgerStatus::Cancelled));
        assert!(r.can_transition_to(&TaskLedgerStatus::Failed));
        assert!(!r.can_transition_to(&TaskLedgerStatus::Pending));
    }
}
