use crate::FfiSecret;
use serde_json::Value;
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncConflictDetail, SyncConflictKind,
    SyncConflictResolution, SyncConflictSummary, SyncEntityKey, SyncEntityKind, SyncErrorSnapshot,
    SyncJoinPreview, SyncLifecycleState, SyncOperation, SyncOperationKind, SyncPresetV1,
    SyncProviderDescriptor, SyncRunResult, SyncStatusSnapshot, SyncVersion,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncPresetV1 {
    Content,
    Standard,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncLifecycleStateV1 {
    Disabled,
    Locked,
    Idle,
    Syncing,
    Paused,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncConflictResolutionV1 {
    KeepCurrent,
    UseConflicting,
    KeepBoth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncConflictKindV1 {
    ConcurrentWrite,
    DeleteVsWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncEntityKindV1 {
    Tag,
    /// Compatibility only: persisted v3 queues may still contain project rows.
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
    AutomationProfile,
    AutomationRule,
    CredentialProfile,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncErrorSnapshotV1 {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncStatusSnapshotV1 {
    pub state: FfiSyncLifecycleStateV1,
    pub provider_id: Option<String>,
    pub vault_id: Option<String>,
    pub preset: Option<FfiSyncPresetV1>,
    pub last_success_at_ms: Option<u64>,
    pub pending_operation_count: u64,
    pub conflict_count: u64,
    pub next_retry_at_ms: Option<u64>,
    pub last_error: Option<FfiSyncErrorSnapshotV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncRunResultV1 {
    pub pulled_segment_count: u64,
    pub pulled_checkpoint_count: u64,
    pub pushed_segment_count: u64,
    pub applied_operation_count: u64,
    pub published_operation_count: u64,
    pub conflict_count: u64,
    pub checkpoint_published: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncJoinPreviewV1 {
    pub local_operation_count: u64,
    pub remote_operation_count: u64,
    pub projected_conflict_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncProviderDescriptorV1 {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncEntityKeyV1 {
    pub kind: FfiSyncEntityKindV1,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncConflictSummaryV1 {
    pub conflict_id: String,
    pub kind: FfiSyncConflictKindV1,
    pub entity: FfiSyncEntityKeyV1,
    pub field: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHybridLogicalClockV1 {
    pub physical_ms: u64,
    pub logical: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncVersionV1 {
    pub clock: FfiHybridLogicalClockV1,
    pub device_id: String,
    pub operation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncCausalContextV1 {
    pub observed_sequences: HashMap<String, u64>,
}

/// `value_json` is the one dynamic leaf: a `SetField` carries an arbitrary
/// entity field value whose schema belongs to that entity, not to Sync.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSyncOperationKindV1 {
    SetField { field: String, value_json: String },
    DeleteEntity,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncOperationV1 {
    pub operation_id: String,
    pub source_device_id: String,
    pub source_sequence: u64,
    pub causal_context: FfiSyncCausalContextV1,
    pub version: FfiSyncVersionV1,
    pub entity: FfiSyncEntityKeyV1,
    pub kind: FfiSyncOperationKindV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncConflictDetailV1 {
    pub summary: FfiSyncConflictSummaryV1,
    pub current: FfiSyncOperationV1,
    pub conflicting: FfiSyncOperationV1,
}

/// `configuration_json` stays a dynamic leaf: Sync is provider-neutral by
/// design, so the configuration shape belongs to the selected provider.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncProviderInputV1 {
    pub provider_id: String,
    pub configuration_json: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FfiSyncCreateRequestV1 {
    pub provider: FfiSyncProviderInputV1,
    pub preset: FfiSyncPresetV1,
    pub master_password: Arc<FfiSecret>,
    pub create_recovery_key: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FfiSyncJoinRequestV1 {
    pub provider: FfiSyncProviderInputV1,
    pub vault_id: String,
    pub master_password: Arc<FfiSecret>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FfiSyncUnlockRequestV1 {
    pub provider_password: Arc<FfiSecret>,
    pub master_password: Option<Arc<FfiSecret>>,
    pub recovery_key: Option<Arc<FfiSecret>>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FfiSyncChangePasswordRequestV1 {
    pub current_master_password: Arc<FfiSecret>,
    pub next_master_password: Arc<FfiSecret>,
}

/// `recovery_key` is returned once at creation and never again, so it crosses
/// as an opaque secret rather than a printable field.
#[derive(Clone, Debug, uniffi::Record)]
pub struct FfiSyncCreateResultV1 {
    pub vault_id: String,
    pub device_id: String,
    pub recovery_key: Option<Arc<FfiSecret>>,
    pub status: FfiSyncStatusSnapshotV1,
}

// ----------------------------------------------------------------- to ffi ---

impl From<SyncPresetV1> for FfiSyncPresetV1 {
    fn from(value: SyncPresetV1) -> Self {
        match value {
            SyncPresetV1::Content => Self::Content,
            SyncPresetV1::Standard => Self::Standard,
            SyncPresetV1::Full => Self::Full,
        }
    }
}

impl From<FfiSyncPresetV1> for SyncPresetV1 {
    fn from(value: FfiSyncPresetV1) -> Self {
        match value {
            FfiSyncPresetV1::Content => Self::Content,
            FfiSyncPresetV1::Standard => Self::Standard,
            FfiSyncPresetV1::Full => Self::Full,
        }
    }
}

impl From<SyncLifecycleState> for FfiSyncLifecycleStateV1 {
    fn from(value: SyncLifecycleState) -> Self {
        match value {
            SyncLifecycleState::Disabled => Self::Disabled,
            SyncLifecycleState::Locked => Self::Locked,
            SyncLifecycleState::Idle => Self::Idle,
            SyncLifecycleState::Syncing => Self::Syncing,
            SyncLifecycleState::Paused => Self::Paused,
            SyncLifecycleState::Error => Self::Error,
        }
    }
}

impl From<FfiSyncConflictResolutionV1> for SyncConflictResolution {
    fn from(value: FfiSyncConflictResolutionV1) -> Self {
        match value {
            FfiSyncConflictResolutionV1::KeepCurrent => Self::KeepCurrent,
            FfiSyncConflictResolutionV1::UseConflicting => Self::UseConflicting,
            FfiSyncConflictResolutionV1::KeepBoth => Self::KeepBoth,
        }
    }
}

impl From<SyncConflictKind> for FfiSyncConflictKindV1 {
    fn from(value: SyncConflictKind) -> Self {
        match value {
            SyncConflictKind::ConcurrentWrite => Self::ConcurrentWrite,
            SyncConflictKind::DeleteVsWrite => Self::DeleteVsWrite,
        }
    }
}

impl From<SyncEntityKind> for FfiSyncEntityKindV1 {
    fn from(value: SyncEntityKind) -> Self {
        match value {
            SyncEntityKind::Tag => Self::Tag,
            SyncEntityKind::Project => Self::Project,
            SyncEntityKind::HistoryItem => Self::HistoryItem,
            SyncEntityKind::HistoryTranscript => Self::HistoryTranscript,
            SyncEntityKind::HistorySummary => Self::HistorySummary,
            SyncEntityKind::TranscriptSnapshot => Self::TranscriptSnapshot,
            SyncEntityKind::Setting => Self::Setting,
            SyncEntityKind::SummaryTemplate => Self::SummaryTemplate,
            SyncEntityKind::PolishPreset => Self::PolishPreset,
            SyncEntityKind::VocabularySet => Self::VocabularySet,
            SyncEntityKind::VocabularyRule => Self::VocabularyRule,
            SyncEntityKind::SpeakerProfile => Self::SpeakerProfile,
            SyncEntityKind::AutomationProfile => Self::AutomationProfile,
            SyncEntityKind::AutomationRule => Self::AutomationRule,
            SyncEntityKind::CredentialProfile => Self::CredentialProfile,
        }
    }
}

impl From<SyncErrorSnapshot> for FfiSyncErrorSnapshotV1 {
    fn from(value: SyncErrorSnapshot) -> Self {
        Self {
            code: value.code,
            message: value.message,
            retryable: value.retryable,
        }
    }
}

impl From<SyncStatusSnapshot> for FfiSyncStatusSnapshotV1 {
    fn from(value: SyncStatusSnapshot) -> Self {
        Self {
            state: value.state.into(),
            provider_id: value.provider_id,
            vault_id: value.vault_id,
            preset: value.preset.map(Into::into),
            last_success_at_ms: value.last_success_at_ms,
            pending_operation_count: value.pending_operation_count,
            conflict_count: value.conflict_count,
            next_retry_at_ms: value.next_retry_at_ms,
            last_error: value.last_error.map(Into::into),
        }
    }
}

impl From<SyncRunResult> for FfiSyncRunResultV1 {
    fn from(value: SyncRunResult) -> Self {
        Self {
            pulled_segment_count: value.pulled_segment_count,
            pulled_checkpoint_count: value.pulled_checkpoint_count,
            pushed_segment_count: value.pushed_segment_count,
            applied_operation_count: value.applied_operation_count,
            published_operation_count: value.published_operation_count,
            conflict_count: value.conflict_count,
            checkpoint_published: value.checkpoint_published,
        }
    }
}

impl From<SyncJoinPreview> for FfiSyncJoinPreviewV1 {
    fn from(value: SyncJoinPreview) -> Self {
        Self {
            local_operation_count: value.local_operation_count,
            remote_operation_count: value.remote_operation_count,
            projected_conflict_count: value.projected_conflict_count,
        }
    }
}

impl From<SyncProviderDescriptor> for FfiSyncProviderDescriptorV1 {
    fn from(value: SyncProviderDescriptor) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
        }
    }
}

impl From<SyncEntityKey> for FfiSyncEntityKeyV1 {
    fn from(value: SyncEntityKey) -> Self {
        Self {
            kind: value.kind.into(),
            id: value.id,
        }
    }
}

impl From<SyncConflictSummary> for FfiSyncConflictSummaryV1 {
    fn from(value: SyncConflictSummary) -> Self {
        Self {
            conflict_id: value.conflict_id,
            kind: value.kind.into(),
            entity: value.entity.into(),
            field: value.field,
            created_at_ms: value.created_at_ms,
        }
    }
}

impl From<HybridLogicalClock> for FfiHybridLogicalClockV1 {
    fn from(value: HybridLogicalClock) -> Self {
        Self {
            physical_ms: value.physical_ms,
            logical: value.logical,
        }
    }
}

impl From<SyncVersion> for FfiSyncVersionV1 {
    fn from(value: SyncVersion) -> Self {
        Self {
            clock: value.clock.into(),
            device_id: value.device_id,
            operation_id: value.operation_id,
        }
    }
}

impl From<SyncCausalContext> for FfiSyncCausalContextV1 {
    fn from(value: SyncCausalContext) -> Self {
        Self {
            observed_sequences: value.observed_sequences.into_iter().collect(),
        }
    }
}

/// Serializing the dynamic `SetField` value can only fail on a non-finite
/// float, which `serde_json::Value` cannot represent, so the error is surfaced
/// rather than silently replaced with a placeholder.
pub(crate) fn sync_operation_to_ffi(
    value: SyncOperation,
) -> Result<FfiSyncOperationV1, serde_json::Error> {
    let kind = match value.kind {
        SyncOperationKind::SetField { field, value } => FfiSyncOperationKindV1::SetField {
            field,
            value_json: serde_json::to_string(&value)?,
        },
        SyncOperationKind::DeleteEntity => FfiSyncOperationKindV1::DeleteEntity,
    };
    Ok(FfiSyncOperationV1 {
        operation_id: value.operation_id,
        source_device_id: value.source_device_id,
        source_sequence: value.source_sequence,
        causal_context: value.causal_context.into(),
        version: value.version.into(),
        entity: value.entity.into(),
        kind,
    })
}

pub(crate) fn sync_conflict_detail_to_ffi(
    value: SyncConflictDetail,
) -> Result<FfiSyncConflictDetailV1, serde_json::Error> {
    Ok(FfiSyncConflictDetailV1 {
        summary: value.summary.into(),
        current: sync_operation_to_ffi(value.current)?,
        conflicting: sync_operation_to_ffi(value.conflicting)?,
    })
}

// ---------------------------------------------------------------- to core ---

pub(crate) fn provider_configuration_from_ffi(
    value: &FfiSyncProviderInputV1,
) -> Result<Value, serde_json::Error> {
    serde_json::from_str(&value.configuration_json)
}

#[allow(dead_code)]
pub(crate) fn causal_context_from_ffi(value: FfiSyncCausalContextV1) -> SyncCausalContext {
    SyncCausalContext {
        observed_sequences: value
            .observed_sequences
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn operation(kind: SyncOperationKind) -> SyncOperation {
        SyncOperation {
            operation_id: "op-1".to_string(),
            source_device_id: "device-a".to_string(),
            source_sequence: 7,
            causal_context: SyncCausalContext {
                observed_sequences: [("device-b".to_string(), 3_u64)].into_iter().collect(),
            },
            version: SyncVersion {
                clock: HybridLogicalClock {
                    physical_ms: 1_700_000_000_000,
                    logical: 2,
                },
                device_id: "device-a".to_string(),
                operation_id: "op-1".to_string(),
            },
            entity: SyncEntityKey {
                kind: SyncEntityKind::Tag,
                id: "tag-1".to_string(),
            },
            kind,
        }
    }

    #[test]
    fn status_snapshot_carries_every_field_across_the_boundary() {
        let status = SyncStatusSnapshot {
            state: SyncLifecycleState::Paused,
            provider_id: Some("webdav".to_string()),
            vault_id: Some("vault-1".to_string()),
            preset: Some(SyncPresetV1::Full),
            last_success_at_ms: Some(42),
            pending_operation_count: 5,
            conflict_count: 2,
            next_retry_at_ms: Some(99),
            last_error: Some(SyncErrorSnapshot {
                code: "E_NET".to_string(),
                message: "offline".to_string(),
                retryable: true,
            }),
        };

        let ffi: FfiSyncStatusSnapshotV1 = status.into();

        assert_eq!(ffi.state, FfiSyncLifecycleStateV1::Paused);
        assert_eq!(ffi.preset, Some(FfiSyncPresetV1::Full));
        assert_eq!(ffi.provider_id.as_deref(), Some("webdav"));
        assert_eq!(ffi.vault_id.as_deref(), Some("vault-1"));
        assert_eq!(ffi.last_success_at_ms, Some(42));
        assert_eq!(ffi.pending_operation_count, 5);
        assert_eq!(ffi.conflict_count, 2);
        assert_eq!(ffi.next_retry_at_ms, Some(99));
        let error = ffi.last_error.expect("last error");
        assert_eq!(error.code, "E_NET");
        assert!(error.retryable);
    }

    #[test]
    fn set_field_operations_keep_their_dynamic_value_as_canonical_json() {
        let ffi = sync_operation_to_ffi(operation(SyncOperationKind::SetField {
            field: "name".to_string(),
            value: json!({"nested": [1, 2]}),
        }))
        .unwrap();

        assert_eq!(ffi.source_sequence, 7);
        assert_eq!(ffi.version.clock.logical, 2);
        assert_eq!(ffi.entity.kind, FfiSyncEntityKindV1::Tag);
        assert_eq!(
            ffi.causal_context.observed_sequences.get("device-b"),
            Some(&3)
        );
        match ffi.kind {
            FfiSyncOperationKindV1::SetField { field, value_json } => {
                assert_eq!(field, "name");
                // The leaf must survive as a parseable document, not a debug string.
                assert_eq!(
                    serde_json::from_str::<Value>(&value_json).unwrap(),
                    json!({"nested": [1, 2]})
                );
            }
            other => panic!("expected SetField, got {other:?}"),
        }
    }

    #[test]
    fn delete_operations_carry_no_value_leaf() {
        let ffi = sync_operation_to_ffi(operation(SyncOperationKind::DeleteEntity)).unwrap();
        assert_eq!(ffi.kind, FfiSyncOperationKindV1::DeleteEntity);
    }

    #[test]
    fn credential_requests_never_print_their_secret() {
        let request = FfiSyncCreateRequestV1 {
            provider: FfiSyncProviderInputV1 {
                provider_id: "webdav".to_string(),
                configuration_json: "{}".to_string(),
            },
            preset: FfiSyncPresetV1::Standard,
            master_password: FfiSecret::new("hunter2".to_string()),
            create_recovery_key: true,
        };

        let rendered = format!("{request:?}");

        assert!(!rendered.contains("hunter2"), "secret leaked: {rendered}");
        assert!(rendered.contains("<redacted>"));
        assert_eq!(request.master_password.expose(), "hunter2");
    }

    #[test]
    fn provider_configuration_rejects_a_malformed_leaf() {
        let input = FfiSyncProviderInputV1 {
            provider_id: "webdav".to_string(),
            configuration_json: "{".to_string(),
        };

        assert!(provider_configuration_from_ffi(&input).is_err());
    }
}
