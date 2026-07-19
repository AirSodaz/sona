use super::asr_streaming_mapper::transcript_segment_to_ffi;
use crate::{
    FfiSpeakerAttribution, FfiSpeakerCandidate, FfiSpeakerTag, FfiStringPatchV1,
    FfiTranscriptSegment, FfiTranscriptTiming, FfiTranscriptTimingLevel, FfiTranscriptTimingSource,
    FfiTranscriptTimingUnit,
};
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryItemMetaPatch, HistoryReplaceTagAssignmentsRequest,
    HistoryTrashItemsRequest, HistoryUpdateItemMetaRequest, HistoryUpdateTagAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
use sona_core::history::{
    HistoryAudioStatus, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSearchRange,
    HistoryWorkspaceSearchSnippet, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    LiveRecordingDraftResult, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
};
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment, TranscriptTiming,
    TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit,
};
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryItemKindV1 {
    Batch,
    Recording,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryItemStatusV1 {
    Draft,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryAudioStatusV1 {
    Available,
    Missing,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryDraftSourceV1 {
    LiveRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryDraftSourcePatchV1 {
    Unchanged,
    Clear,
    Set { value: FfiHistoryDraftSourceV1 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTranscriptSnapshotReasonV1 {
    Polish,
    Translate,
    Retranscribe,
    Restore,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryWorkspaceScopeV1 {
    All,
    Untagged,
    Tag { tag_id: String },
    Trash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryWorkspaceFilterTypeV1 {
    All,
    Recording,
    Batch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryWorkspaceDateFilterV1 {
    All,
    Today,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiHistoryWorkspaceSortOrderV1 {
    Newest,
    Oldest,
    DurationDesc,
    DurationAsc,
    TitleAsc,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryItemRecordV1 {
    pub id: String,
    pub timestamp: u64,
    pub duration: f64,
    pub audio_path: String,
    pub audio_status: FfiHistoryAudioStatusV1,
    pub transcript_path: String,
    pub title: String,
    pub preview_text: String,
    pub icon: Option<String>,
    pub kind: FfiHistoryItemKindV1,
    pub search_content: String,
    pub tag_ids: Vec<String>,
    pub deleted_at: Option<u64>,
    pub status: FfiHistoryItemStatusV1,
    pub draft_source: Option<FfiHistoryDraftSourceV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryCreateLiveDraftRequestV1 {
    pub id: Option<String>,
    pub audio_extension: String,
    pub tag_ids: Vec<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryCompleteLiveDraftRequestV1 {
    pub history_id: String,
    pub segments: Vec<FfiTranscriptSegment>,
    pub duration: f64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryUpdateTranscriptRequestV1 {
    pub history_id: String,
    pub segments: Vec<FfiTranscriptSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryDeleteItemsRequestV1 {
    pub ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistorySaveRecordingRequestV1 {
    pub segments: Vec<FfiTranscriptSegment>,
    pub duration: f64,
    pub tag_ids: Vec<String>,
    pub audio_bytes: Option<Vec<u8>>,
    pub native_audio_path: Option<String>,
    pub audio_extension: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistorySaveImportedFileRequestV1 {
    pub id: Option<String>,
    pub source_path: String,
    pub segments: Vec<FfiTranscriptSegment>,
    pub duration: f64,
    pub tag_ids: Vec<String>,
    pub converted_source_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryTrashItemsRequestV1 {
    pub ids: Vec<String>,
    pub deleted_at: u64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryCreateTranscriptSnapshotRequestV1 {
    pub history_id: String,
    pub reason: FfiTranscriptSnapshotReasonV1,
    pub segments: Vec<FfiTranscriptSegment>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryItemMetaPatchV1 {
    pub timestamp: Option<u64>,
    pub duration: Option<f64>,
    pub audio_path: Option<String>,
    pub audio_status: Option<FfiHistoryAudioStatusV1>,
    pub transcript_path: Option<String>,
    pub title: Option<String>,
    pub preview_text: Option<String>,
    pub icon: FfiStringPatchV1,
    pub kind: Option<FfiHistoryItemKindV1>,
    pub search_content: Option<String>,
    pub status: Option<FfiHistoryItemStatusV1>,
    pub draft_source: FfiHistoryDraftSourcePatchV1,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryUpdateItemMetaRequestV1 {
    pub history_id: String,
    pub updates: FfiHistoryItemMetaPatchV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryUpdateTagAssignmentsRequestV1 {
    pub ids: Vec<String>,
    pub add_tag_ids: Vec<String>,
    pub remove_tag_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryReplaceTagAssignmentsRequestV1 {
    pub ids: Vec<String>,
    pub tag_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTranscriptSnapshotMetadataV1 {
    pub id: String,
    pub history_id: String,
    pub reason: FfiTranscriptSnapshotReasonV1,
    pub created_at: u64,
    pub segment_count: u64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranscriptSnapshotRecordV1 {
    pub metadata: FfiTranscriptSnapshotMetadataV1,
    pub segments: Vec<FfiTranscriptSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryWorkspaceQueryRequestV1 {
    pub scope: FfiHistoryWorkspaceScopeV1,
    pub query: String,
    pub filter_type: FfiHistoryWorkspaceFilterTypeV1,
    pub date_filter: FfiHistoryWorkspaceDateFilterV1,
    pub sort_order: FfiHistoryWorkspaceSortOrderV1,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryWorkspaceSearchRangeV1 {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryWorkspaceSearchSnippetV1 {
    pub text: String,
    pub highlight_start: u64,
    pub highlight_end: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryWorkspaceItemSearchMatchV1 {
    pub matched_field: String,
    pub title_match: Option<FfiHistoryWorkspaceSearchRangeV1>,
    pub display_snippet: FfiHistoryWorkspaceSearchSnippetV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistorySearchMatchEntryV1 {
    pub history_id: String,
    pub search_match: Option<FfiHistoryWorkspaceItemSearchMatchV1>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryWorkspaceSummaryV1 {
    pub total_items: u64,
    pub total_duration: f64,
    pub latest_timestamp: Option<u64>,
    pub recording_count: u64,
    pub batch_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryTagCountEntryV1 {
    pub tag_id: String,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiHistoryWorkspaceItemCountsV1 {
    pub untagged: u64,
    pub trash: u64,
    pub by_tag_id: Vec<FfiHistoryTagCountEntryV1>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiHistoryWorkspaceQueryResultV1 {
    pub filtered_items: Vec<FfiHistoryItemRecordV1>,
    pub search_matches: Vec<FfiHistorySearchMatchEntryV1>,
    pub filtered_item_count: u64,
    pub has_more: bool,
    pub summary: FfiHistoryWorkspaceSummaryV1,
    pub item_counts: FfiHistoryWorkspaceItemCountsV1,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLiveRecordingDraftResultV1 {
    pub item: FfiHistoryItemRecordV1,
    pub audio_absolute_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryMapperError {
    field: &'static str,
    reason: String,
}

impl Display for HistoryMapperError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Invalid {}: {}", self.field, self.reason)
    }
}

impl From<FfiHistoryCreateLiveDraftRequestV1> for HistoryCreateLiveDraftRequest {
    fn from(value: FfiHistoryCreateLiveDraftRequestV1) -> Self {
        Self {
            id: value.id,
            audio_extension: value.audio_extension,
            tag_ids: value.tag_ids,
            icon: value.icon,
        }
    }
}

impl TryFrom<FfiHistoryCompleteLiveDraftRequestV1> for HistoryCompleteLiveDraftRequest {
    type Error = HistoryMapperError;

    fn try_from(value: FfiHistoryCompleteLiveDraftRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            history_id: value.history_id,
            segments: history_transcript_segments_from_ffi(value.segments)?,
            duration: value.duration,
        })
    }
}

impl TryFrom<FfiHistoryUpdateTranscriptRequestV1> for HistoryUpdateTranscriptRequest {
    type Error = HistoryMapperError;

    fn try_from(value: FfiHistoryUpdateTranscriptRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            history_id: value.history_id,
            segments: history_transcript_segments_from_ffi(value.segments)?,
        })
    }
}

impl From<FfiHistoryDeleteItemsRequestV1> for HistoryDeleteItemsRequest {
    fn from(value: FfiHistoryDeleteItemsRequestV1) -> Self {
        Self { ids: value.ids }
    }
}

impl TryFrom<FfiHistorySaveRecordingRequestV1> for HistorySaveRecordingRequest {
    type Error = HistoryMapperError;

    fn try_from(value: FfiHistorySaveRecordingRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            segments: history_transcript_segments_from_ffi(value.segments)?,
            duration: value.duration,
            tag_ids: value.tag_ids,
            audio_bytes: value.audio_bytes,
            native_audio_path: value.native_audio_path,
            audio_extension: value.audio_extension,
        })
    }
}

impl TryFrom<FfiHistorySaveImportedFileRequestV1> for HistorySaveImportedFileRequest {
    type Error = HistoryMapperError;

    fn try_from(value: FfiHistorySaveImportedFileRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            source_path: value.source_path,
            segments: history_transcript_segments_from_ffi(value.segments)?,
            duration: value.duration,
            tag_ids: value.tag_ids,
            converted_source_path: value.converted_source_path,
        })
    }
}

impl From<FfiHistoryTrashItemsRequestV1> for HistoryTrashItemsRequest {
    fn from(value: FfiHistoryTrashItemsRequestV1) -> Self {
        Self {
            ids: value.ids,
            deleted_at: value.deleted_at,
        }
    }
}

impl TryFrom<FfiHistoryCreateTranscriptSnapshotRequestV1>
    for HistoryCreateTranscriptSnapshotRequest
{
    type Error = HistoryMapperError;

    fn try_from(value: FfiHistoryCreateTranscriptSnapshotRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            history_id: value.history_id,
            reason: value.reason.into(),
            segments: history_transcript_segments_from_ffi(value.segments)?,
        })
    }
}

impl From<FfiHistoryUpdateItemMetaRequestV1> for HistoryUpdateItemMetaRequest {
    fn from(value: FfiHistoryUpdateItemMetaRequestV1) -> Self {
        Self {
            history_id: value.history_id,
            updates: value.updates.into(),
        }
    }
}

impl From<FfiHistoryItemMetaPatchV1> for HistoryItemMetaPatch {
    fn from(value: FfiHistoryItemMetaPatchV1) -> Self {
        Self {
            timestamp: value.timestamp,
            duration: value.duration,
            audio_path: value.audio_path,
            audio_status: value.audio_status.map(Into::into),
            transcript_path: value.transcript_path,
            title: value.title,
            preview_text: value.preview_text,
            icon: value.icon.into_core(),
            kind: value.kind.map(Into::into),
            search_content: value.search_content,
            status: value.status.map(Into::into),
            draft_source: match value.draft_source {
                FfiHistoryDraftSourcePatchV1::Unchanged => None,
                FfiHistoryDraftSourcePatchV1::Clear => Some(None),
                FfiHistoryDraftSourcePatchV1::Set { value } => Some(Some(value.into())),
            },
        }
    }
}

impl From<FfiHistoryUpdateTagAssignmentsRequestV1> for HistoryUpdateTagAssignmentsRequest {
    fn from(value: FfiHistoryUpdateTagAssignmentsRequestV1) -> Self {
        Self {
            ids: value.ids,
            add_tag_ids: value.add_tag_ids,
            remove_tag_ids: value.remove_tag_ids,
        }
    }
}

impl From<FfiHistoryReplaceTagAssignmentsRequestV1> for HistoryReplaceTagAssignmentsRequest {
    fn from(value: FfiHistoryReplaceTagAssignmentsRequestV1) -> Self {
        Self {
            ids: value.ids,
            tag_ids: value.tag_ids,
        }
    }
}

impl TryFrom<FfiHistoryWorkspaceQueryRequestV1> for HistoryWorkspaceQueryRequest {
    type Error = HistoryMapperError;

    fn try_from(value: FfiHistoryWorkspaceQueryRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            scope: value.scope.into(),
            query: value.query,
            filter_type: value.filter_type.into(),
            date_filter: value.date_filter.into(),
            sort_order: value.sort_order.into(),
            limit: input_usize("history workspace limit", value.limit)?,
            offset: input_usize("history workspace offset", value.offset)?,
        })
    }
}

impl From<FfiHistoryWorkspaceScopeV1> for HistoryWorkspaceScope {
    fn from(value: FfiHistoryWorkspaceScopeV1) -> Self {
        match value {
            FfiHistoryWorkspaceScopeV1::All => Self::All,
            FfiHistoryWorkspaceScopeV1::Untagged => Self::Untagged,
            FfiHistoryWorkspaceScopeV1::Tag { tag_id } => Self::Tag { tag_id },
            FfiHistoryWorkspaceScopeV1::Trash => Self::Trash,
        }
    }
}

impl From<FfiHistoryWorkspaceFilterTypeV1> for HistoryWorkspaceFilterType {
    fn from(value: FfiHistoryWorkspaceFilterTypeV1) -> Self {
        match value {
            FfiHistoryWorkspaceFilterTypeV1::All => Self::All,
            FfiHistoryWorkspaceFilterTypeV1::Recording => Self::Recording,
            FfiHistoryWorkspaceFilterTypeV1::Batch => Self::Batch,
        }
    }
}

impl From<FfiHistoryWorkspaceDateFilterV1> for HistoryWorkspaceDateFilter {
    fn from(value: FfiHistoryWorkspaceDateFilterV1) -> Self {
        match value {
            FfiHistoryWorkspaceDateFilterV1::All => Self::All,
            FfiHistoryWorkspaceDateFilterV1::Today => Self::Today,
            FfiHistoryWorkspaceDateFilterV1::Week => Self::Week,
            FfiHistoryWorkspaceDateFilterV1::Month => Self::Month,
        }
    }
}

impl From<FfiHistoryWorkspaceSortOrderV1> for HistoryWorkspaceSortOrder {
    fn from(value: FfiHistoryWorkspaceSortOrderV1) -> Self {
        match value {
            FfiHistoryWorkspaceSortOrderV1::Newest => Self::Newest,
            FfiHistoryWorkspaceSortOrderV1::Oldest => Self::Oldest,
            FfiHistoryWorkspaceSortOrderV1::DurationDesc => Self::DurationDesc,
            FfiHistoryWorkspaceSortOrderV1::DurationAsc => Self::DurationAsc,
            FfiHistoryWorkspaceSortOrderV1::TitleAsc => Self::TitleAsc,
        }
    }
}

impl From<HistoryItemRecord> for FfiHistoryItemRecordV1 {
    fn from(value: HistoryItemRecord) -> Self {
        Self {
            id: value.id,
            timestamp: value.timestamp,
            duration: value.duration,
            audio_path: value.audio_path,
            audio_status: value.audio_status.into(),
            transcript_path: value.transcript_path,
            title: value.title,
            preview_text: value.preview_text,
            icon: value.icon,
            kind: value.kind.into(),
            search_content: value.search_content,
            tag_ids: value.tag_ids,
            deleted_at: value.deleted_at,
            status: value.status.into(),
            draft_source: value.draft_source.map(Into::into),
        }
    }
}

impl From<HistoryItemKind> for FfiHistoryItemKindV1 {
    fn from(value: HistoryItemKind) -> Self {
        match value {
            HistoryItemKind::Batch => Self::Batch,
            HistoryItemKind::Recording => Self::Recording,
        }
    }
}

impl From<FfiHistoryItemKindV1> for HistoryItemKind {
    fn from(value: FfiHistoryItemKindV1) -> Self {
        match value {
            FfiHistoryItemKindV1::Batch => Self::Batch,
            FfiHistoryItemKindV1::Recording => Self::Recording,
        }
    }
}

impl From<HistoryItemStatus> for FfiHistoryItemStatusV1 {
    fn from(value: HistoryItemStatus) -> Self {
        match value {
            HistoryItemStatus::Draft => Self::Draft,
            HistoryItemStatus::Complete => Self::Complete,
        }
    }
}

impl From<FfiHistoryItemStatusV1> for HistoryItemStatus {
    fn from(value: FfiHistoryItemStatusV1) -> Self {
        match value {
            FfiHistoryItemStatusV1::Draft => Self::Draft,
            FfiHistoryItemStatusV1::Complete => Self::Complete,
        }
    }
}

impl From<HistoryAudioStatus> for FfiHistoryAudioStatusV1 {
    fn from(value: HistoryAudioStatus) -> Self {
        match value {
            HistoryAudioStatus::Available => Self::Available,
            HistoryAudioStatus::Missing => Self::Missing,
            HistoryAudioStatus::Removed => Self::Removed,
        }
    }
}

impl From<FfiHistoryAudioStatusV1> for HistoryAudioStatus {
    fn from(value: FfiHistoryAudioStatusV1) -> Self {
        match value {
            FfiHistoryAudioStatusV1::Available => Self::Available,
            FfiHistoryAudioStatusV1::Missing => Self::Missing,
            FfiHistoryAudioStatusV1::Removed => Self::Removed,
        }
    }
}

impl From<HistoryDraftSource> for FfiHistoryDraftSourceV1 {
    fn from(value: HistoryDraftSource) -> Self {
        match value {
            HistoryDraftSource::LiveRecord => Self::LiveRecord,
        }
    }
}

impl From<FfiHistoryDraftSourceV1> for HistoryDraftSource {
    fn from(value: FfiHistoryDraftSourceV1) -> Self {
        match value {
            FfiHistoryDraftSourceV1::LiveRecord => Self::LiveRecord,
        }
    }
}

impl From<FfiTranscriptSnapshotReasonV1> for TranscriptSnapshotReason {
    fn from(value: FfiTranscriptSnapshotReasonV1) -> Self {
        match value {
            FfiTranscriptSnapshotReasonV1::Polish => Self::Polish,
            FfiTranscriptSnapshotReasonV1::Translate => Self::Translate,
            FfiTranscriptSnapshotReasonV1::Retranscribe => Self::Retranscribe,
            FfiTranscriptSnapshotReasonV1::Restore => Self::Restore,
        }
    }
}

impl From<TranscriptSnapshotReason> for FfiTranscriptSnapshotReasonV1 {
    fn from(value: TranscriptSnapshotReason) -> Self {
        match value {
            TranscriptSnapshotReason::Polish => Self::Polish,
            TranscriptSnapshotReason::Translate => Self::Translate,
            TranscriptSnapshotReason::Retranscribe => Self::Retranscribe,
            TranscriptSnapshotReason::Restore => Self::Restore,
        }
    }
}

impl From<TranscriptSnapshotMetadata> for FfiTranscriptSnapshotMetadataV1 {
    fn from(value: TranscriptSnapshotMetadata) -> Self {
        Self {
            id: value.id,
            history_id: value.history_id,
            reason: value.reason.into(),
            created_at: value.created_at,
            segment_count: value.segment_count,
        }
    }
}

impl From<TranscriptSnapshotRecord> for FfiTranscriptSnapshotRecordV1 {
    fn from(value: TranscriptSnapshotRecord) -> Self {
        Self {
            metadata: value.metadata.into(),
            segments: history_transcript_segments_to_ffi(value.segments),
        }
    }
}

impl From<LiveRecordingDraftResult> for FfiLiveRecordingDraftResultV1 {
    fn from(value: LiveRecordingDraftResult) -> Self {
        Self {
            item: value.item.into(),
            audio_absolute_path: value.audio_absolute_path,
        }
    }
}

pub(crate) fn history_workspace_result_to_ffi(
    value: HistoryWorkspaceQueryResult,
) -> Result<FfiHistoryWorkspaceQueryResultV1, HistoryMapperError> {
    Ok(FfiHistoryWorkspaceQueryResultV1 {
        filtered_items: value.filtered_items.into_iter().map(Into::into).collect(),
        search_matches: value
            .search_match_by_item_id
            .into_iter()
            .map(|(history_id, search_match)| {
                history_search_match_entry_to_ffi(history_id, search_match)
            })
            .collect::<Result<_, _>>()?,
        filtered_item_count: output_u64("filtered item count", value.filtered_item_count)?,
        has_more: value.has_more,
        summary: history_summary_to_ffi(value.summary)?,
        item_counts: history_item_counts_to_ffi(value.item_counts)?,
    })
}

pub(crate) fn history_transcript_to_ffi(
    value: Option<Vec<TranscriptSegment>>,
) -> Option<Vec<FfiTranscriptSegment>> {
    value.map(history_transcript_segments_to_ffi)
}

pub(crate) fn history_transcript_segments_to_ffi(
    segments: Vec<TranscriptSegment>,
) -> Vec<FfiTranscriptSegment> {
    segments.iter().map(transcript_segment_to_ffi).collect()
}

pub(crate) fn history_transcript_segments_from_ffi(
    values: Vec<FfiTranscriptSegment>,
) -> Result<Vec<TranscriptSegment>, HistoryMapperError> {
    values
        .into_iter()
        .map(history_transcript_segment_from_ffi)
        .collect()
}

fn history_transcript_segment_from_ffi(
    value: FfiTranscriptSegment,
) -> Result<TranscriptSegment, HistoryMapperError> {
    Ok(TranscriptSegment {
        id: value.id,
        text: value.text,
        start: value.start,
        end: value.end,
        is_final: value.is_final,
        timing: value.timing.map(history_timing_from_ffi),
        tokens: value.tokens,
        timestamps: value.timestamps,
        durations: value.durations,
        translation: value.translation,
        speaker: value.speaker.map(history_speaker_from_ffi),
        speaker_attribution: value
            .speaker_attribution
            .map(history_attribution_from_ffi)
            .transpose()?,
    })
}

fn history_timing_from_ffi(value: FfiTranscriptTiming) -> TranscriptTiming {
    TranscriptTiming {
        level: match value.level {
            FfiTranscriptTimingLevel::Token => TranscriptTimingLevel::Token,
            FfiTranscriptTimingLevel::Segment => TranscriptTimingLevel::Segment,
        },
        source: match value.source {
            FfiTranscriptTimingSource::Model => TranscriptTimingSource::Model,
            FfiTranscriptTimingSource::Derived => TranscriptTimingSource::Derived,
        },
        units: value
            .units
            .into_iter()
            .map(|unit: FfiTranscriptTimingUnit| TranscriptTimingUnit {
                text: unit.text,
                start: unit.start,
                end: unit.end,
            })
            .collect(),
    }
}

fn history_speaker_from_ffi(value: FfiSpeakerTag) -> SpeakerTag {
    SpeakerTag {
        id: value.id,
        label: value.label,
        kind: value.kind,
        score: value.score,
    }
}

fn history_attribution_from_ffi(
    value: FfiSpeakerAttribution,
) -> Result<SpeakerAttribution, HistoryMapperError> {
    Ok(SpeakerAttribution {
        group_id: value.group_id,
        anonymous_label: value.anonymous_label,
        state: value.state,
        source: value.source,
        confidence: value.confidence,
        candidates: value
            .candidates
            .into_iter()
            .map(history_candidate_from_ffi)
            .collect::<Result<_, _>>()?,
    })
}

fn history_candidate_from_ffi(
    value: FfiSpeakerCandidate,
) -> Result<SpeakerCandidate, HistoryMapperError> {
    Ok(SpeakerCandidate {
        profile_id: value.profile_id,
        profile_name: value.profile_name,
        score: value.score,
        rank: input_usize("speaker attribution candidate rank", value.rank)?,
    })
}

fn history_search_match_entry_to_ffi(
    history_id: String,
    value: Option<HistoryWorkspaceItemSearchMatch>,
) -> Result<FfiHistorySearchMatchEntryV1, HistoryMapperError> {
    Ok(FfiHistorySearchMatchEntryV1 {
        history_id,
        search_match: value.map(history_search_match_to_ffi).transpose()?,
    })
}

fn history_search_match_to_ffi(
    value: HistoryWorkspaceItemSearchMatch,
) -> Result<FfiHistoryWorkspaceItemSearchMatchV1, HistoryMapperError> {
    Ok(FfiHistoryWorkspaceItemSearchMatchV1 {
        matched_field: value.matched_field,
        title_match: value
            .title_match
            .map(history_search_range_to_ffi)
            .transpose()?,
        display_snippet: history_search_snippet_to_ffi(value.display_snippet)?,
    })
}

fn history_search_range_to_ffi(
    value: HistoryWorkspaceSearchRange,
) -> Result<FfiHistoryWorkspaceSearchRangeV1, HistoryMapperError> {
    Ok(FfiHistoryWorkspaceSearchRangeV1 {
        start: output_u64("history search range start", value.start)?,
        end: output_u64("history search range end", value.end)?,
    })
}

fn history_search_snippet_to_ffi(
    value: HistoryWorkspaceSearchSnippet,
) -> Result<FfiHistoryWorkspaceSearchSnippetV1, HistoryMapperError> {
    Ok(FfiHistoryWorkspaceSearchSnippetV1 {
        text: value.text,
        highlight_start: output_u64("history search highlight start", value.highlight_start)?,
        highlight_end: output_u64("history search highlight end", value.highlight_end)?,
    })
}

fn history_summary_to_ffi(
    value: HistoryWorkspaceSummary,
) -> Result<FfiHistoryWorkspaceSummaryV1, HistoryMapperError> {
    Ok(FfiHistoryWorkspaceSummaryV1 {
        total_items: output_u64("history total items", value.total_items)?,
        total_duration: value.total_duration,
        latest_timestamp: value.latest_timestamp,
        recording_count: output_u64("history recording count", value.recording_count)?,
        batch_count: output_u64("history batch count", value.batch_count)?,
    })
}

fn history_item_counts_to_ffi(
    value: HistoryWorkspaceItemCounts,
) -> Result<FfiHistoryWorkspaceItemCountsV1, HistoryMapperError> {
    Ok(FfiHistoryWorkspaceItemCountsV1 {
        untagged: output_u64("untagged history count", value.untagged)?,
        trash: output_u64("trash history count", value.trash)?,
        by_tag_id: value
            .by_tag_id
            .into_iter()
            .map(|(tag_id, count)| {
                Ok(FfiHistoryTagCountEntryV1 {
                    tag_id,
                    count: output_u64("history tag count", count)?,
                })
            })
            .collect::<Result<_, HistoryMapperError>>()?,
    })
}

fn input_usize(field: &'static str, value: u64) -> Result<usize, HistoryMapperError> {
    if value > i64::MAX as u64 {
        return Err(HistoryMapperError {
            field,
            reason: "value exceeds the supported i64 range".to_string(),
        });
    }
    usize::try_from(value).map_err(|_| HistoryMapperError {
        field,
        reason: "value exceeds the host usize range".to_string(),
    })
}

fn output_u64(field: &'static str, value: usize) -> Result<u64, HistoryMapperError> {
    u64::try_from(value).map_err(|_| HistoryMapperError {
        field,
        reason: "value exceeds the UniFFI u64 range".to_string(),
    })
}
