package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemKind
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.HistorySearchMatch
import com.sona.android.application.library.HistorySortOrder
import com.sona.android.application.library.HistoryWorkspaceCounts
import com.sona.android.application.library.HistoryWorkspacePage
import com.sona.android.application.library.HistoryWorkspacePort
import com.sona.android.application.library.HistoryWorkspaceQuery
import com.sona.android.application.library.HistoryWorkspaceSummary
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.TranscriptSnapshotDetail
import com.sona.android.application.library.TranscriptSnapshotReason
import com.sona.android.application.llm.LlmHistorySummaryPort
import com.sona.android.application.llm.LlmSummary
import com.sona.android.application.recording.CompleteLiveDraftRequest
import com.sona.android.application.recording.CreateLiveDraftRequest
import com.sona.android.application.recording.HistoryRecordingSummary
import com.sona.android.application.recording.ImportedRecordingHistoryPort
import com.sona.android.application.recording.RecordingDestination
import com.sona.android.application.recording.RecordingDraft
import com.sona.android.application.recording.RecordingHistoryPort
import com.sona.android.application.recording.SaveImportedRecordingRequest
import com.sona.android.application.recording.TranscriptSegment
import uniffi.sona_uniffi_bind.FfiHistoryAudioStatusV1
import uniffi.sona_uniffi_bind.FfiHistoryCompleteLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryCreateLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryDeleteItemsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryItemRecordV1
import uniffi.sona_uniffi_bind.FfiHistoryItemKindV1
import uniffi.sona_uniffi_bind.FfiHistoryItemMetaPatchV1
import uniffi.sona_uniffi_bind.FfiHistoryItemStatusV1
import uniffi.sona_uniffi_bind.FfiHistorySaveImportedFileRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateTranscriptRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryTrashItemsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateItemMetaRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateTagAssignmentsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryDraftSourcePatchV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceDateFilterV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceFilterTypeV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceQueryRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceScopeV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceSortOrderV1
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.FfiStringPatchV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotMetadataV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1
import uniffi.sona_uniffi_bind.FfiHistoryCreateTranscriptSnapshotRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryCommitTranscriptEditRequestV1
import uniffi.sona_uniffi_bind.FfiHistorySummaryPayloadV1
import uniffi.sona_uniffi_bind.FfiTranscriptSummaryRecordV1
import kotlin.math.roundToLong

class UniffiRecordingHistoryAdapter internal constructor(
    private val appDataDir: String,
    private val bindings: UniffiHistoryBindings,
    private val onLocalChange: () -> Unit = {},
) : RecordingHistoryPort, HistoryWorkspacePort, ImportedRecordingHistoryPort, LlmHistorySummaryPort {
    constructor(appDataDir: String, onLocalChange: () -> Unit = {}) :
        this(appDataDir, GeneratedUniffiHistoryBindings, onLocalChange)

    init {
        require(appDataDir.isNotBlank()) { "History app data directory must not be blank." }
    }

    override suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft {
        val response = bindings.createLiveDraft(
            appDataDir,
            FfiHistoryCreateLiveDraftRequestV1(
                id = request.recordingId,
                audioExtension = request.audioExtension,
                tagIds = emptyList(),
                icon = null,
            ),
        )
        return RecordingDraft(
            historyId = response.item.id,
            destination = RecordingDestination(response.audioAbsolutePath),
        )
    }

    override suspend fun checkpointTranscript(
        historyId: String,
        segments: List<TranscriptSegment>,
    ) {
        bindings.updateTranscript(
            appDataDir,
            FfiHistoryUpdateTranscriptRequestV1(
                historyId = historyId,
                segments = segments.map(TranscriptSegment::toFfi),
            ),
        )
        onLocalChange()
    }

    override suspend fun completeLiveDraft(
        request: CompleteLiveDraftRequest,
    ): HistoryRecordingSummary {
        val response = bindings.completeLiveDraft(
            appDataDir,
            FfiHistoryCompleteLiveDraftRequestV1(
                historyId = request.historyId,
                segments = request.segments.map(TranscriptSegment::toFfi),
                duration = request.durationMillis / 1_000.0,
            ),
        )
        onLocalChange()
        return HistoryRecordingSummary(historyId = response.id)
    }

    override suspend fun deleteDraft(historyId: String) {
        bindings.purgeItems(
            appDataDir,
            FfiHistoryDeleteItemsRequestV1(ids = listOf(historyId)),
        )
        onLocalChange()
    }

    override suspend fun query(request: HistoryWorkspaceQuery): HistoryWorkspacePage {
        require(request.offset >= 0) { "History offset must not be negative." }
        require(request.limit in 1..MAX_LIBRARY_PAGE_SIZE) {
            "History limit must be between 1 and $MAX_LIBRARY_PAGE_SIZE."
        }
        val response = bindings.queryWorkspace(
            appDataDir,
            FfiHistoryWorkspaceQueryRequestV1(
                scope = request.scope.toFfi(),
                query = request.query,
                filterType = request.filterType.toFfi(),
                dateFilter = request.dateFilter.toFfi(),
                sortOrder = request.sortOrder.toFfi(),
                limit = request.limit.toULong(),
                offset = request.offset.toULong(),
            ),
        )
        val matches = response.searchMatches.associate { entry ->
            entry.historyId to entry.searchMatch?.let { match ->
                HistorySearchMatch(
                    field = match.matchedField,
                    snippet = match.displaySnippet.text,
                    highlightStart = match.displaySnippet.highlightStart.toIntChecked("Search highlight start"),
                    highlightEnd = match.displaySnippet.highlightEnd.toIntChecked("Search highlight end"),
                )
            }
        }
        return HistoryWorkspacePage(
            items = response.filteredItems.map { it.toApplication(matches[it.id]) },
            filteredItemCount = response.filteredItemCount.toLongChecked("Filtered item count"),
            hasMore = response.hasMore,
            summary = HistoryWorkspaceSummary(
                totalItems = response.summary.totalItems.toLongChecked("History total"),
                totalDurationMillis = (response.summary.totalDuration.coerceAtLeast(0.0) * 1_000.0).roundToLong(),
                latestTimestampEpochMillis = response.summary.latestTimestamp?.toLongChecked("Latest timestamp"),
                recordingCount = response.summary.recordingCount.toLongChecked("Recording count"),
                batchCount = response.summary.batchCount.toLongChecked("Batch count"),
            ),
            counts = HistoryWorkspaceCounts(
                untagged = response.itemCounts.untagged.toLongChecked("Untagged count"),
                trash = response.itemCounts.trash.toLongChecked("Trash count"),
                byTagId = response.itemCounts.byTagId.associate { it.tagId to it.count.toLongChecked("Tag count") },
            ),
        )
    }

    override suspend fun loadTranscript(historyId: String): List<TranscriptSegment> {
        require(historyId.isNotBlank()) { "History ID must not be blank." }
        return bindings.loadTranscript(appDataDir, historyId)
            .orEmpty()
            .map(FfiTranscriptSegment::toApplication)
    }

    override suspend fun contains(historyId: String): Boolean {
        require(historyId.isNotBlank()) { "History ID must not be blank." }
        return bindings.loadTranscript(appDataDir, historyId) != null
    }

    override suspend fun saveImported(
        request: SaveImportedRecordingRequest,
    ): HistoryRecordingSummary {
        require(request.historyId.isNotBlank()) { "History ID must not be blank." }
        require(request.normalizedWavPath.isNotBlank()) { "Imported WAV path must not be blank." }
        val sourceName = request.displayName
            .substringAfterLast('/')
            .substringAfterLast('\\')
            .ifBlank { "Imported audio.wav" }
        val response = bindings.saveImported(
            appDataDir,
            FfiHistorySaveImportedFileRequestV1(
                id = request.historyId,
                sourcePath = sourceName,
                segments = request.segments.map(TranscriptSegment::toFfi),
                duration = request.durationMillis.coerceAtLeast(0L) / 1_000.0,
                tagIds = emptyList(),
                convertedSourcePath = request.normalizedWavPath,
            ),
        )
        onLocalChange()
        return HistoryRecordingSummary(response.id)
    }

    override suspend fun updateTranscript(
        historyId: String,
        segments: List<TranscriptSegment>,
    ) {
        checkpointTranscript(historyId, segments)
    }

    override suspend fun updateTitle(historyId: String, title: String) {
        require(historyId.isNotBlank()) { "History ID must not be blank." }
        bindings.updateItemMeta(
            appDataDir,
            FfiHistoryUpdateItemMetaRequestV1(
                historyId,
                FfiHistoryItemMetaPatchV1(
                    timestamp = null,
                    duration = null,
                    audioPath = null,
                    audioStatus = null,
                    transcriptPath = null,
                    title = title.trim(),
                    previewText = null,
                    icon = FfiStringPatchV1.Unchanged,
                    kind = null,
                    searchContent = null,
                    status = null,
                    draftSource = FfiHistoryDraftSourcePatchV1.Unchanged,
                ),
            ),
        )
        onLocalChange()
    }

    override suspend fun updateTags(ids: List<String>, addTagIds: List<String>, removeTagIds: List<String>) {
        require(ids.isNotEmpty() && ids.none(String::isBlank)) { "History IDs must not be empty." }
        bindings.updateTagAssignments(
            appDataDir,
            FfiHistoryUpdateTagAssignmentsRequestV1(ids, addTagIds, removeTagIds),
        )
        onLocalChange()
    }

    override suspend fun trash(ids: List<String>, deletedAtEpochMillis: Long) {
        bindings.trashItems(appDataDir, FfiHistoryTrashItemsRequestV1(ids, deletedAtEpochMillis.toULong()))
        onLocalChange()
    }

    override suspend fun restore(ids: List<String>) {
        bindings.restoreItems(appDataDir, FfiHistoryDeleteItemsRequestV1(ids))
        onLocalChange()
    }

    override suspend fun purge(ids: List<String>) {
        bindings.purgeItems(appDataDir, FfiHistoryDeleteItemsRequestV1(ids))
        onLocalChange()
    }

    override suspend fun listSnapshots(historyId: String): List<TranscriptSnapshot> =
        bindings.listSnapshots(appDataDir, historyId).map(FfiTranscriptSnapshotMetadataV1::toApplication)

    override suspend fun loadSnapshot(historyId: String, snapshotId: String): TranscriptSnapshotDetail? =
        bindings.loadSnapshot(appDataDir, historyId, snapshotId)?.let {
            TranscriptSnapshotDetail(
                metadata = it.metadata.toApplication(),
                segments = it.segments.map(FfiTranscriptSegment::toApplication),
            )
        }

    override suspend fun loadSummary(historyId: String): LlmSummary? = bindings.loadSummary(appDataDir, historyId)?.record?.let {
        LlmSummary(it.templateId, it.content, it.generatedAt, it.sourceFingerprint)
    }

    override suspend fun saveSummary(historyId: String, summary: LlmSummary) {
        bindings.saveSummary(appDataDir, historyId, FfiHistorySummaryPayloadV1(summary.templateId, FfiTranscriptSummaryRecordV1(summary.templateId, summary.content, summary.generatedAt, summary.sourceFingerprint)))
        onLocalChange()
    }

    override suspend fun deleteSummary(historyId: String) { bindings.deleteSummary(appDataDir, historyId); onLocalChange() }

    override suspend fun createSnapshot(historyId: String, reason: TranscriptSnapshotReason) {
        val segments = loadTranscript(historyId)
        bindings.createSnapshot(appDataDir, FfiHistoryCreateTranscriptSnapshotRequestV1(historyId, reason.toFfi(), segments.map(TranscriptSegment::toFfi)))
    }

    override suspend fun commitTranscript(historyId: String, segments: List<TranscriptSegment>) {
        val current = loadTranscript(historyId)
        bindings.commitTranscriptEdit(appDataDir, FfiHistoryCommitTranscriptEditRequestV1(historyId, "android-llm", current.map(TranscriptSegment::toFfi), segments.map(TranscriptSegment::toFfi)))
        onLocalChange()
    }
}

private const val MAX_LIBRARY_PAGE_SIZE = 200

private fun TranscriptSnapshotReason.toFfi() = when (this) {
    TranscriptSnapshotReason.POLISH -> FfiTranscriptSnapshotReasonV1.POLISH
    TranscriptSnapshotReason.TRANSLATE -> FfiTranscriptSnapshotReasonV1.TRANSLATE
    TranscriptSnapshotReason.RETRANSCRIBE -> FfiTranscriptSnapshotReasonV1.RETRANSCRIBE
    TranscriptSnapshotReason.RESTORE -> FfiTranscriptSnapshotReasonV1.RESTORE
    TranscriptSnapshotReason.MANUAL_EDIT -> FfiTranscriptSnapshotReasonV1.MANUAL_EDIT
}

private fun FfiHistoryItemRecordV1.toApplication(searchMatch: HistorySearchMatch? = null): HistoryItem = HistoryItem(
    historyId = id,
    title = title,
    timestampEpochMillis = timestamp.toLongChecked("History timestamp"),
    durationMillis = (duration.coerceAtLeast(0.0) * 1_000.0).roundToLong(),
    previewText = previewText,
    status = when (status) {
        FfiHistoryItemStatusV1.DRAFT -> HistoryItemStatus.DRAFT
        FfiHistoryItemStatusV1.COMPLETE -> HistoryItemStatus.COMPLETE
    },
    kind = when (kind) {
        FfiHistoryItemKindV1.RECORDING -> HistoryItemKind.RECORDING
        FfiHistoryItemKindV1.BATCH -> HistoryItemKind.BATCH
    },
    tagIds = tagIds,
    deletedAtEpochMillis = deletedAt?.toLongChecked("History deleted timestamp"),
    audioPath = audioPath,
    audioAvailable = audioStatus == FfiHistoryAudioStatusV1.AVAILABLE && audioPath.isNotBlank(),
    icon = icon,
    searchMatch = searchMatch,
)

private fun HistoryScope.toFfi(): FfiHistoryWorkspaceScopeV1 = when (this) {
    HistoryScope.All -> FfiHistoryWorkspaceScopeV1.All
    HistoryScope.Untagged -> FfiHistoryWorkspaceScopeV1.Untagged
    HistoryScope.Trash -> FfiHistoryWorkspaceScopeV1.Trash
    is HistoryScope.Tag -> FfiHistoryWorkspaceScopeV1.Tag(tagId)
}

private fun HistoryFilterType.toFfi() = when (this) {
    HistoryFilterType.ALL -> FfiHistoryWorkspaceFilterTypeV1.ALL
    HistoryFilterType.RECORDING -> FfiHistoryWorkspaceFilterTypeV1.RECORDING
    HistoryFilterType.BATCH -> FfiHistoryWorkspaceFilterTypeV1.BATCH
}

private fun HistoryDateFilter.toFfi() = when (this) {
    HistoryDateFilter.ALL -> FfiHistoryWorkspaceDateFilterV1.ALL
    HistoryDateFilter.TODAY -> FfiHistoryWorkspaceDateFilterV1.TODAY
    HistoryDateFilter.WEEK -> FfiHistoryWorkspaceDateFilterV1.WEEK
    HistoryDateFilter.MONTH -> FfiHistoryWorkspaceDateFilterV1.MONTH
}

private fun HistorySortOrder.toFfi() = when (this) {
    HistorySortOrder.NEWEST -> FfiHistoryWorkspaceSortOrderV1.NEWEST
    HistorySortOrder.OLDEST -> FfiHistoryWorkspaceSortOrderV1.OLDEST
    HistorySortOrder.DURATION_DESC -> FfiHistoryWorkspaceSortOrderV1.DURATION_DESC
    HistorySortOrder.DURATION_ASC -> FfiHistoryWorkspaceSortOrderV1.DURATION_ASC
    HistorySortOrder.TITLE_ASC -> FfiHistoryWorkspaceSortOrderV1.TITLE_ASC
}

private fun FfiTranscriptSnapshotMetadataV1.toApplication() = TranscriptSnapshot(
    id = id,
    historyId = historyId,
    reason = when (reason) {
        FfiTranscriptSnapshotReasonV1.POLISH -> TranscriptSnapshotReason.POLISH
        FfiTranscriptSnapshotReasonV1.TRANSLATE -> TranscriptSnapshotReason.TRANSLATE
        FfiTranscriptSnapshotReasonV1.RETRANSCRIBE -> TranscriptSnapshotReason.RETRANSCRIBE
        FfiTranscriptSnapshotReasonV1.RESTORE -> TranscriptSnapshotReason.RESTORE
        FfiTranscriptSnapshotReasonV1.MANUAL_EDIT -> TranscriptSnapshotReason.MANUAL_EDIT
    },
    createdAtEpochMillis = createdAt.toLongChecked("Snapshot timestamp"),
    segmentCount = segmentCount.toLongChecked("Snapshot segment count"),
)

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}

private fun ULong.toIntChecked(label: String): Int {
    require(this <= Int.MAX_VALUE.toULong()) { "$label exceeds the Android Int range." }
    return toInt()
}
