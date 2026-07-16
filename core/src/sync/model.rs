use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SyncError;

pub const SYNC_PROTOCOL_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncPresetV1 {
    Content,
    #[default]
    Standard,
    Full,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncLifecycleState {
    Disabled,
    Locked,
    Idle,
    Syncing,
    Paused,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncErrorSnapshot {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusSnapshot {
    pub state: SyncLifecycleState,
    pub provider_id: Option<String>,
    pub vault_id: Option<String>,
    pub preset: Option<SyncPresetV1>,
    pub last_success_at_ms: Option<u64>,
    pub pending_operation_count: u64,
    pub conflict_count: u64,
    pub next_retry_at_ms: Option<u64>,
    pub last_error: Option<SyncErrorSnapshot>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunResult {
    pub pulled_segment_count: u64,
    pub pulled_checkpoint_count: u64,
    pub pushed_segment_count: u64,
    pub applied_operation_count: u64,
    pub published_operation_count: u64,
    pub conflict_count: u64,
    pub checkpoint_published: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncJoinPreview {
    pub local_operation_count: u64,
    pub remote_operation_count: u64,
    pub projected_conflict_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncProviderDescriptor {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictResolution {
    KeepCurrent,
    UseConflicting,
    KeepBoth,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub struct HybridLogicalClock {
    pub physical_ms: u64,
    pub logical: u32,
}

impl HybridLogicalClock {
    pub fn tick(self, now_ms: u64) -> Self {
        if now_ms > self.physical_ms {
            Self {
                physical_ms: now_ms,
                logical: 0,
            }
        } else {
            Self {
                physical_ms: self.physical_ms,
                logical: self.logical.saturating_add(1),
            }
        }
    }

    pub fn observe(self, remote: Self, now_ms: u64) -> Self {
        let physical_ms = self.physical_ms.max(remote.physical_ms).max(now_ms);
        let logical = match (
            physical_ms == self.physical_ms,
            physical_ms == remote.physical_ms,
        ) {
            (true, true) => self.logical.max(remote.logical).saturating_add(1),
            (true, false) => self.logical.saturating_add(1),
            (false, true) => remote.logical.saturating_add(1),
            (false, false) => 0,
        };
        Self {
            physical_ms,
            logical,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncVersion {
    pub clock: HybridLogicalClock,
    pub device_id: String,
    pub operation_id: String,
}

impl Ord for SyncVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.clock
            .cmp(&other.clock)
            .then_with(|| self.device_id.cmp(&other.device_id))
            .then_with(|| self.operation_id.cmp(&other.operation_id))
    }
}

impl PartialOrd for SyncVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncCausalContext {
    pub observed_sequences: BTreeMap<String, u64>,
}

impl SyncCausalContext {
    pub fn observes(&self, device_id: &str, sequence: u64) -> bool {
        self.observed_sequences
            .get(device_id)
            .is_some_and(|observed| *observed >= sequence)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SyncEntityKind {
    Tag,
    // Compatibility only: persisted v3 queues may still contain project operations.
    Project,
    HistoryItem,
    HistoryTranscript,
    HistorySummary,
    TranscriptSnapshot,
    Setting,
    SummaryTemplate,
    PolishPreset,
    VocabularySet,
    VocabularyRule,
    SpeakerProfile,
    AutomationRule,
    CredentialProfile,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct SyncEntityKey {
    pub kind: SyncEntityKind,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyncOperationKind {
    SetField { field: String, value: Value },
    DeleteEntity,
}

impl SyncOperationKind {
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::SetField { field, .. } => Some(field),
            Self::DeleteEntity => None,
        }
    }

    pub fn has_same_value(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::SetField {
                    field: left_field,
                    value: left_value,
                },
                Self::SetField {
                    field: right_field,
                    value: right_value,
                },
            ) => left_field == right_field && left_value == right_value,
            (Self::DeleteEntity, Self::DeleteEntity) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncOperation {
    pub operation_id: String,
    pub source_device_id: String,
    pub source_sequence: u64,
    pub causal_context: SyncCausalContext,
    pub version: SyncVersion,
    pub entity: SyncEntityKey,
    pub kind: SyncOperationKind,
}

impl SyncOperation {
    pub fn observes(&self, other: &Self) -> bool {
        if self.source_device_id == other.source_device_id {
            self.source_sequence > other.source_sequence
                || (self.source_sequence == other.source_sequence && self.version > other.version)
        } else {
            self.causal_context
                .observes(&other.source_device_id, other.source_sequence)
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictKind {
    ConcurrentWrite,
    DeleteVsWrite,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub kind: SyncConflictKind,
    pub winner: SyncOperation,
    pub loser: SyncOperation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictSummary {
    pub conflict_id: String,
    pub kind: SyncConflictKind,
    pub entity: SyncEntityKey,
    pub field: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictDetail {
    pub summary: SyncConflictSummary,
    pub current: SyncOperation,
    pub conflicting: SyncOperation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncMergeOutcome {
    pub winner: SyncOperation,
    pub conflict: Option<SyncConflict>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct SyncObjectKey(String);

impl SyncObjectKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, SyncError> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.contains(':')
        {
            return Err(SyncError::InvalidObjectKey(value));
        }
        if value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(SyncError::InvalidObjectKey(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SyncObjectKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct SyncObjectPrefix(String);

impl SyncObjectPrefix {
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, SyncError> {
        let value = value.into();
        if value.is_empty() {
            return Ok(Self::root());
        }
        SyncObjectKey::parse(value.clone())?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&SyncObjectKey> for SyncObjectPrefix {
    fn from(key: &SyncObjectKey) -> Self {
        Self(key.as_str().to_string())
    }
}
