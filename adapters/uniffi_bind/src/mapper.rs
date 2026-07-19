#[path = "mapper/asr_mapper.rs"]
mod asr_mapper;
#[path = "mapper/asr_streaming_mapper.rs"]
mod asr_streaming_mapper;
#[path = "mapper/automation_mapper.rs"]
mod automation_mapper;
#[path = "mapper/config_mapper.rs"]
mod config_mapper;
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
#[path = "mapper/tag_mapper.rs"]
mod tag_mapper;
#[path = "mapper/task_ledger_mapper.rs"]
mod task_ledger_mapper;

pub use asr_mapper::*;
pub use asr_streaming_mapper::*;
pub use automation_mapper::*;
pub use config_mapper::*;
pub use history_mapper::{
    FfiHistoryAudioStatusV1, FfiHistoryCompleteLiveDraftRequestV1,
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
    FfiHistoryWorkspaceSummaryV1, FfiLiveRecordingDraftResultV1, FfiTranscriptSnapshotMetadataV1,
    FfiTranscriptSnapshotReasonV1, FfiTranscriptSnapshotRecordV1,
};
pub(crate) use history_mapper::{history_transcript_to_ffi, history_workspace_result_to_ffi};
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
pub use tag_mapper::*;
pub use task_ledger_mapper::*;
