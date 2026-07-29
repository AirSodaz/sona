package com.sona.android.application.library

import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

class HistoryWorkspacePortContractTest {
    @Test
    fun `history workspace can be consumed without platform types`() = runTest {
        val item = HistoryItem(
            historyId = "history-1",
            title = "Recording 1",
            timestampEpochMillis = 1_725_000_000_000,
            durationMillis = 2_500,
            previewText = "Hello",
            status = HistoryItemStatus.DRAFT,
            kind = HistoryItemKind.RECORDING,
        )
        val transcript = listOf(
            TranscriptSegment(
                id = "segment-1",
                text = "Hello",
                startSeconds = 0.0,
                endSeconds = 2.5,
                isFinal = true,
            ),
        )
        val page = HistoryWorkspacePage(
            items = listOf(item),
            filteredItemCount = 1,
            hasMore = false,
            summary = HistoryWorkspaceSummary(1, 2_500, item.timestampEpochMillis, 1, 0),
            counts = HistoryWorkspaceCounts(untagged = 1, trash = 0, byTagId = emptyMap()),
        )
        val port = object : HistoryWorkspacePort {
            override suspend fun query(request: HistoryWorkspaceQuery) = page
            override suspend fun loadTranscript(historyId: String) = transcript
            override suspend fun updateTitle(historyId: String, title: String) = Unit
            override suspend fun updateTags(ids: List<String>, addTagIds: List<String>, removeTagIds: List<String>) = Unit
            override suspend fun trash(ids: List<String>, deletedAtEpochMillis: Long) = Unit
            override suspend fun restore(ids: List<String>) = Unit
            override suspend fun purge(ids: List<String>) = Unit
            override suspend fun listSnapshots(historyId: String) = emptyList<TranscriptSnapshot>()
            override suspend fun loadSnapshot(historyId: String, snapshotId: String) = null
        }

        assertEquals(page, port.query(HistoryWorkspaceQuery()))
        assertEquals(transcript, port.loadTranscript("history-1"))
    }
}
