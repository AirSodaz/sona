#[path = "mapper/asr_mapper.rs"]
mod asr_mapper;
#[path = "mapper/asr_streaming_mapper.rs"]
mod asr_streaming_mapper;
#[path = "mapper/automation_mapper.rs"]
mod automation_mapper;
#[path = "mapper/backup_mapper.rs"]
mod backup_mapper;
#[path = "mapper/config_mapper.rs"]
mod config_mapper;
#[path = "mapper/dashboard_mapper.rs"]
mod dashboard_mapper;
#[path = "mapper/diagnostics_mapper.rs"]
mod diagnostics_mapper;
#[path = "mapper/export_mapper.rs"]
mod export_mapper;
#[path = "mapper/history_mapper.rs"]
mod history_mapper;
#[path = "mapper/llm_mapper.rs"]
mod llm_mapper;
#[path = "mapper/llm_runtime_mapper.rs"]
mod llm_runtime_mapper;
#[path = "mapper/llm_task_mapper.rs"]
mod llm_task_mapper;
#[path = "mapper/model_mapper.rs"]
mod model_mapper;
#[path = "mapper/recovery_mapper.rs"]
mod recovery_mapper;
#[path = "mapper/runtime_mapper.rs"]
mod runtime_mapper;
#[path = "mapper/secret_mapper.rs"]
mod secret_mapper;
#[path = "mapper/storage_usage_mapper.rs"]
mod storage_usage_mapper;
#[path = "mapper/sync_mapper.rs"]
mod sync_mapper;
#[path = "mapper/tag_mapper.rs"]
mod tag_mapper;
#[path = "mapper/task_ledger_mapper.rs"]
mod task_ledger_mapper;

pub use asr_mapper::*;
pub use asr_streaming_mapper::*;
pub use automation_mapper::*;
pub(crate) use backup_mapper::prepared_backup_import_to_ffi;
pub use backup_mapper::{
    FfiBackupApplyResultV1, FfiBackupManifestCountsV1, FfiBackupManifestScopesV1,
    FfiBackupManifestV1, FfiPreparedBackupImportV1,
};
pub use config_mapper::*;
pub use dashboard_mapper::{
    FfiContentStatsV1, FfiContentTrendPointV1, FfiDashboardSnapshotV1, FfiDashboardUsageBucketV1,
    FfiLlmUsageDashboardStatsV1, FfiOverviewStatsV1, FfiSpeakerLeaderV1, FfiSpeakerStatsV1,
    FfiUsageBreakdownV1, FfiUsageTrendPointV1,
};
pub use diagnostics_mapper::{
    FfiAsrRuntimeMetricsSnapshotV1, FfiDiagnosticsConfigV1, FfiDiagnosticsDeviceOptionV1,
    FfiDiagnosticsDeviceProbeV1, FfiDiagnosticsInputV1, FfiDiagnosticsModelRuleV1,
    FfiDiagnosticsModelRulesV1, FfiDiagnosticsModelSummaryV1, FfiDiagnosticsPathStatusesV1,
    FfiDiagnosticsSelectedModelsV1, FfiDiagnosticsSnapshotV1, FfiRuntimeEnvironmentStatusV1,
    FfiVoiceTypingReadinessV1,
};
pub(crate) use export_mapper::export_request_from_ffi;
pub use export_mapper::{
    FfiExportFormatV1, FfiExportModeV1, FfiExportTranscriptFileRequestV1,
    FfiExportTranscriptFileResultV1,
};
pub use history_mapper::{
    FfiAudioSourceV1, FfiHistoryAudioStatusV1, FfiHistoryCommitTranscriptEditRequestV1,
    FfiHistoryCommitTranscriptEditResultV1, FfiHistoryCompleteLiveDraftRequestV1,
    FfiHistoryCreateLiveDraftRequestV1, FfiHistoryCreateTranscriptSnapshotRequestV1,
    FfiHistoryDeleteItemsRequestV1, FfiHistoryDraftSourcePatchV1, FfiHistoryDraftSourceV1,
    FfiHistoryItemKindV1, FfiHistoryItemMetaPatchV1, FfiHistoryItemRecordV1,
    FfiHistoryItemStatusV1, FfiHistoryReplaceTagAssignmentsRequestV1,
    FfiHistorySaveImportedFileRequestV1, FfiHistorySaveRecordingRequestV1,
    FfiHistorySearchMatchEntryV1, FfiHistoryTagCountEntryV1, FfiHistoryTrashItemsRequestV1,
    FfiHistoryUpdateItemMetaRequestV1, FfiHistoryUpdateTagAssignmentsRequestV1,
    FfiHistoryUpdateTranscriptRequestV1, FfiHistoryWorkspaceDateFilterV1,
    FfiHistoryWorkspaceFilterTypeV1, FfiHistoryWorkspaceItemCountsV1,
    FfiHistoryWorkspaceItemSearchMatchV1, FfiHistoryWorkspaceQueryRequestV1,
    FfiHistoryWorkspaceQueryResultV1, FfiHistoryWorkspaceScopeV1, FfiHistoryWorkspaceSearchRangeV1,
    FfiHistoryWorkspaceSearchSnippetV1, FfiHistoryWorkspaceSortOrderV1,
    FfiHistoryWorkspaceSummaryV1, FfiLiveRecordingDraftResultV1, FfiTranscriptEditOperationV1,
    FfiTranscriptSnapshotMetadataV1, FfiTranscriptSnapshotReasonV1, FfiTranscriptSnapshotRecordV1,
};
pub(crate) use history_mapper::{
    history_transcript_segments_from_ffi, history_transcript_segments_to_ffi,
    history_transcript_to_ffi, history_workspace_result_to_ffi,
};
pub use llm_mapper::*;
pub use llm_runtime_mapper::*;
pub use llm_task_mapper::*;
pub use model_mapper::*;
pub use recovery_mapper::{
    FfiRecoveredQueueItemV1, FfiRecoveredTranscriptSegmentV1, FfiRecoveredTranscriptTimingUnitV1,
    FfiRecoveredTranscriptTimingV1, FfiRecoveryFileStatV1, FfiRecoveryItemInputV1,
    FfiRecoveryItemStageV1, FfiRecoveryQueueStatusV1, FfiRecoveryResolutionV1,
    FfiRecoverySnapshotV1, FfiRecoverySourceV1,
};
pub use runtime_mapper::*;
pub use secret_mapper::FfiSecret;
pub use storage_usage_mapper::{
    FfiAudioUsageCategoryV1, FfiDatabaseUsageCategoryV1, FfiFileUsageCategoryV1,
    FfiSqliteIndexUsageEntryV1, FfiSqliteUsageSummaryV1, FfiStorageUsageCategoriesV1,
    FfiStorageUsageSnapshotV1, FfiWebviewCacheUsageCategoryV1,
};
pub use sync_mapper::{
    FfiHybridLogicalClockV1, FfiSyncCausalContextV1, FfiSyncChangePasswordRequestV1,
    FfiSyncConflictDetailV1, FfiSyncConflictKindV1, FfiSyncConflictResolutionV1,
    FfiSyncConflictSummaryV1, FfiSyncCreateRequestV1, FfiSyncCreateResultV1, FfiSyncEntityKeyV1,
    FfiSyncEntityKindV1, FfiSyncErrorSnapshotV1, FfiSyncJoinPreviewV1, FfiSyncJoinRequestV1,
    FfiSyncLifecycleStateV1, FfiSyncOperationKindV1, FfiSyncOperationV1, FfiSyncPresetV1,
    FfiSyncProviderDescriptorV1, FfiSyncProviderInputV1, FfiSyncRunResultV1,
    FfiSyncStatusSnapshotV1, FfiSyncUnlockRequestV1, FfiSyncVersionV1,
};
pub(crate) use sync_mapper::{provider_configuration_from_ffi, sync_conflict_detail_to_ffi};
pub use tag_mapper::*;
pub use task_ledger_mapper::*;
