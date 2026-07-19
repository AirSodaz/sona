package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.library.RecordingLibraryItemStatus
import com.sona.android.application.library.RecordingLibraryPage
import com.sona.android.application.library.RecordingLibraryPort
import com.sona.android.application.recording.CompleteLiveDraftRequest
import com.sona.android.application.recording.CreateLiveDraftRequest
import com.sona.android.application.recording.HistoryRecordingSummary
import com.sona.android.application.recording.RecordingDestination
import com.sona.android.application.recording.RecordingDraft
import com.sona.android.application.recording.RecordingHistoryPort
import com.sona.android.application.recording.TranscriptSegment
import uniffi.sona_uniffi_bind.FfiHistoryCompleteLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryCreateLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryDeleteItemsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryItemRecordV1
import uniffi.sona_uniffi_bind.FfiHistoryItemStatusV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateTranscriptRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceDateFilterV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceFilterTypeV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceQueryRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceScopeV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceSortOrderV1
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import kotlin.math.roundToLong

class UniffiRecordingHistoryAdapter internal constructor(
    private val appDataDir: String,
    private val bindings: UniffiHistoryBindings,
) : RecordingHistoryPort, RecordingLibraryPort {
    constructor(appDataDir: String) : this(appDataDir, GeneratedUniffiHistoryBindings)

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
        return HistoryRecordingSummary(historyId = response.id)
    }

    override suspend fun deleteDraft(historyId: String) {
        bindings.purgeItems(
            appDataDir,
            FfiHistoryDeleteItemsRequestV1(ids = listOf(historyId)),
        )
    }

    override suspend fun loadPage(offset: Int, limit: Int): RecordingLibraryPage {
        require(offset >= 0) { "History offset must not be negative." }
        require(limit in 1..MAX_LIBRARY_PAGE_SIZE) {
            "History limit must be between 1 and $MAX_LIBRARY_PAGE_SIZE."
        }
        val response = bindings.queryWorkspace(
            appDataDir,
            FfiHistoryWorkspaceQueryRequestV1(
                scope = FfiHistoryWorkspaceScopeV1.All,
                query = "",
                filterType = FfiHistoryWorkspaceFilterTypeV1.RECORDING,
                dateFilter = FfiHistoryWorkspaceDateFilterV1.ALL,
                sortOrder = FfiHistoryWorkspaceSortOrderV1.NEWEST,
                limit = limit.toULong(),
                offset = offset.toULong(),
            ),
        )
        return RecordingLibraryPage(
            items = response.filteredItems.map(FfiHistoryItemRecordV1::toApplication),
            hasMore = response.hasMore,
        )
    }

    override suspend fun loadTranscript(historyId: String): List<TranscriptSegment> {
        require(historyId.isNotBlank()) { "History ID must not be blank." }
        return bindings.loadTranscript(appDataDir, historyId)
            .orEmpty()
            .map(FfiTranscriptSegment::toApplication)
    }
}

private const val MAX_LIBRARY_PAGE_SIZE = 200

private fun FfiHistoryItemRecordV1.toApplication(): RecordingLibraryItem = RecordingLibraryItem(
    historyId = id,
    title = title,
    timestampEpochMillis = timestamp.toLongChecked("History timestamp"),
    durationMillis = (duration.coerceAtLeast(0.0) * 1_000.0).roundToLong(),
    previewText = previewText,
    status = when (status) {
        FfiHistoryItemStatusV1.DRAFT -> RecordingLibraryItemStatus.DRAFT
        FfiHistoryItemStatusV1.COMPLETE -> RecordingLibraryItemStatus.COMPLETE
    },
    tagIds = tagIds,
    deletedAtEpochMillis = deletedAt?.toLongChecked("History deleted timestamp"),
)

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}
