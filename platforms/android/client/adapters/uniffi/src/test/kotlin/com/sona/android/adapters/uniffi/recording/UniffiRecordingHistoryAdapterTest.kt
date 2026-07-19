package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.library.RecordingLibraryItemStatus
import com.sona.android.application.library.RecordingLibraryPage
import com.sona.android.application.recording.CompleteLiveDraftRequest
import com.sona.android.application.recording.CreateLiveDraftRequest
import com.sona.android.application.recording.RecordingDestination
import com.sona.android.application.recording.RecordingDraft
import com.sona.android.application.recording.SpeakerAttribution
import com.sona.android.application.recording.SpeakerCandidate
import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import com.sona.android.application.recording.TranscriptTimingUnit
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiHistoryAudioStatusV1
import uniffi.sona_uniffi_bind.FfiHistoryCompleteLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryCreateLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryDeleteItemsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryDraftSourceV1
import uniffi.sona_uniffi_bind.FfiHistoryItemKindV1
import uniffi.sona_uniffi_bind.FfiHistoryItemRecordV1
import uniffi.sona_uniffi_bind.FfiHistoryItemStatusV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateTranscriptRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceDateFilterV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceFilterTypeV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceItemCountsV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceQueryRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceQueryResultV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceScopeV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceSortOrderV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceSummaryV1
import uniffi.sona_uniffi_bind.FfiLiveRecordingDraftResultV1
import uniffi.sona_uniffi_bind.FfiTranscriptSegment

class UniffiRecordingHistoryAdapterTest {
    @Test
    fun `maps recording library pages and transcripts across the typed UniFFI boundary`() = runTest {
        val bindings = FakeHistoryBindings()
        val adapter = UniffiRecordingHistoryAdapter("C:/app-data", bindings)

        val page = adapter.loadPage(offset = 20, limit = 30)
        val transcript = adapter.loadTranscript("history-1")

        assertEquals(
            RecordingLibraryPage(
                items = listOf(
                    RecordingLibraryItem(
                        historyId = "history-1",
                        title = "Recording 1",
                        timestampEpochMillis = 1_725_000_000_000,
                        durationMillis = 2_500,
                        previewText = "Hello mobile history",
                        status = RecordingLibraryItemStatus.DRAFT,
                        tagIds = listOf("tag-1"),
                        deletedAtEpochMillis = null,
                    ),
                ),
                hasMore = true,
            ),
            page,
        )
        assertEquals(listOf(fullSegment()), transcript)
        val request = checkNotNull(bindings.queryRequest)
        assertEquals(FfiHistoryWorkspaceScopeV1.All, request.scope)
        assertEquals(FfiHistoryWorkspaceFilterTypeV1.RECORDING, request.filterType)
        assertEquals(FfiHistoryWorkspaceSortOrderV1.NEWEST, request.sortOrder)
        assertEquals(20uL, request.offset)
        assertEquals(30uL, request.limit)
        assertEquals("history-1", bindings.transcriptHistoryId)
    }

    @Test
    fun `maps the history lifecycle across the typed UniFFI boundary`() = runTest {
        val bindings = FakeHistoryBindings()
        val adapter = UniffiRecordingHistoryAdapter("C:/app-data", bindings)
        val segment = fullSegment()

        val draft = adapter.createLiveDraft(CreateLiveDraftRequest("recording-1", "wav"))
        adapter.checkpointTranscript("history-1", listOf(segment))
        val summary = adapter.completeLiveDraft(
            CompleteLiveDraftRequest("history-1", listOf(segment), durationMillis = 2_500),
        )
        adapter.deleteDraft("history-1")

        assertEquals(
            RecordingDraft(
                historyId = "history-1",
                destination = RecordingDestination("C:/app-data/history/history-1.wav"),
            ),
            draft,
        )
        val create = checkNotNull(bindings.createRequest)
        val checkpoint = checkNotNull(bindings.updateRequest)
        val complete = checkNotNull(bindings.completeRequest)
        val delete = checkNotNull(bindings.deleteRequest)
        assertEquals("C:/app-data", bindings.createAppDataDir)
        assertEquals("recording-1", create.id)
        assertEquals("wav", create.audioExtension)
        assertEquals(emptyList<String>(), create.tagIds)
        assertNull(create.icon)
        assertEquals("history-1", checkpoint.historyId)
        assertEquals("segment-1", checkpoint.segments.single().id)
        assertEquals("speaker-1", checkpoint.segments.single().speaker?.id)
        assertEquals(1uL, checkpoint.segments.single().speakerAttribution?.candidates?.single()?.rank)
        assertEquals("history-1", summary.historyId)
        assertEquals(2.5, complete.duration, 0.0)
        assertEquals(1, complete.segments.size)
        assertEquals(listOf("history-1"), delete.ids)
    }

    @Test
    fun `rejects timestamps outside the Android Long range`() {
        val bindings = FakeHistoryBindings().apply {
            queryResponse = queryResult(itemRecord(timestamp = ULong.MAX_VALUE))
        }

        assertThrows(IllegalArgumentException::class.java) {
            runTest {
                UniffiRecordingHistoryAdapter("C:/app-data", bindings)
                    .loadPage(offset = 0, limit = 30)
            }
        }
    }

    @Test
    fun `maps an absent persisted transcript to an empty detail`() = runTest {
        val bindings = FakeHistoryBindings().apply { transcriptResponse = null }

        assertEquals(
            emptyList<TranscriptSegment>(),
            UniffiRecordingHistoryAdapter("C:/app-data", bindings)
                .loadTranscript("history-1"),
        )
    }

    @Test
    fun `accepts a persisted recording with an empty title`() = runTest {
        val bindings = FakeHistoryBindings().apply {
            queryResponse = queryResult(itemRecord(title = ""))
        }

        val page = UniffiRecordingHistoryAdapter("C:/app-data", bindings)
            .loadPage(offset = 0, limit = 30)

        assertEquals("", page.items.single().title)
    }

    @Test
    fun `transcript cancellation remains cancellation`() {
        val bindings = FakeHistoryBindings().apply {
            transcriptFailure = CancellationException("cancelled")
        }

        assertThrows(CancellationException::class.java) {
            runTest {
                UniffiRecordingHistoryAdapter("C:/app-data", bindings)
                    .loadTranscript("history-1")
            }
        }
    }

    private fun fullSegment() = TranscriptSegment(
        id = "segment-1",
        text = "hello",
        startSeconds = 1.25,
        endSeconds = 2.5,
        isFinal = true,
        timing = TranscriptTiming(
            level = TranscriptTimingLevel.TOKEN,
            source = TranscriptTimingSource.MODEL,
            units = listOf(TranscriptTimingUnit("he", 1.25, 1.5)),
        ),
        tokens = listOf("he", "llo"),
        timestamps = listOf(1.25f, 1.75f),
        durations = listOf(0.5f, 0.75f),
        translation = "\u4f60\u597d",
        speaker = SpeakerTag("speaker-1", "Speaker 1", "known", 0.8f),
        speakerAttribution = SpeakerAttribution(
            groupId = "group-1",
            anonymousLabel = "Speaker 1",
            state = "matched",
            source = "embedding",
            confidence = "high",
            candidates = listOf(SpeakerCandidate("profile-1", "Alice", 0.9f, 1uL)),
        ),
    )

    private inner class FakeHistoryBindings : UniffiHistoryBindings {
        var createAppDataDir: String? = null
        var createRequest: FfiHistoryCreateLiveDraftRequestV1? = null
        var updateRequest: FfiHistoryUpdateTranscriptRequestV1? = null
        var completeRequest: FfiHistoryCompleteLiveDraftRequestV1? = null
        var deleteRequest: FfiHistoryDeleteItemsRequestV1? = null
        var queryRequest: FfiHistoryWorkspaceQueryRequestV1? = null
        var transcriptHistoryId: String? = null
        var transcriptFailure: Throwable? = null
        var createResponse = FfiLiveRecordingDraftResultV1(
            item = itemRecord(),
            audioAbsolutePath = "C:/app-data/history/history-1.wav",
        )
        var queryResponse = queryResult(itemRecord())
        var transcriptResponse: List<FfiTranscriptSegment>? = listOf(fullSegment().toFfi())

        override suspend fun createLiveDraft(
            appDataDir: String,
            request: FfiHistoryCreateLiveDraftRequestV1,
        ): FfiLiveRecordingDraftResultV1 {
            createAppDataDir = appDataDir
            createRequest = request
            return createResponse
        }

        override suspend fun updateTranscript(
            appDataDir: String,
            request: FfiHistoryUpdateTranscriptRequestV1,
        ): FfiHistoryItemRecordV1 {
            updateRequest = request
            return itemRecord()
        }

        override suspend fun completeLiveDraft(
            appDataDir: String,
            request: FfiHistoryCompleteLiveDraftRequestV1,
        ): FfiHistoryItemRecordV1 {
            completeRequest = request
            return itemRecord(status = FfiHistoryItemStatusV1.COMPLETE)
        }

        override suspend fun purgeItems(
            appDataDir: String,
            request: FfiHistoryDeleteItemsRequestV1,
        ) {
            deleteRequest = request
        }

        override suspend fun queryWorkspace(
            appDataDir: String,
            request: FfiHistoryWorkspaceQueryRequestV1,
        ): FfiHistoryWorkspaceQueryResultV1 {
            queryRequest = request
            return queryResponse
        }

        override suspend fun loadTranscript(
            appDataDir: String,
            historyId: String,
        ): List<FfiTranscriptSegment>? {
            transcriptHistoryId = historyId
            transcriptFailure?.let { throw it }
            return transcriptResponse
        }
    }

    companion object {
        private fun itemRecord(
            timestamp: ULong = 1_725_000_000_000uL,
            title: String = "Recording 1",
            status: FfiHistoryItemStatusV1 = FfiHistoryItemStatusV1.DRAFT,
        ) = FfiHistoryItemRecordV1(
            id = "history-1",
            timestamp = timestamp,
            duration = 2.5,
            audioPath = "history-1.wav",
            audioStatus = FfiHistoryAudioStatusV1.AVAILABLE,
            transcriptPath = "history-1.json",
            title = title,
            previewText = "Hello mobile history",
            icon = null,
            kind = FfiHistoryItemKindV1.RECORDING,
            searchContent = "",
            tagIds = listOf("tag-1"),
            deletedAt = null,
            status = status,
            draftSource = FfiHistoryDraftSourceV1.LIVE_RECORD,
        )

        private fun queryResult(item: FfiHistoryItemRecordV1) =
            FfiHistoryWorkspaceQueryResultV1(
                filteredItems = listOf(item),
                searchMatches = emptyList(),
                filteredItemCount = 1uL,
                hasMore = true,
                summary = FfiHistoryWorkspaceSummaryV1(
                    totalItems = 1uL,
                    totalDuration = item.duration,
                    latestTimestamp = item.timestamp,
                    recordingCount = 1uL,
                    batchCount = 0uL,
                ),
                itemCounts = FfiHistoryWorkspaceItemCountsV1(
                    untagged = 0uL,
                    trash = 0uL,
                    byTagId = emptyList(),
                ),
            )
    }
}
