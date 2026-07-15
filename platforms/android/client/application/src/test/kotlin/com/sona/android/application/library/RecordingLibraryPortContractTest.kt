package com.sona.android.application.library

import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

class RecordingLibraryPortContractTest {
    @Test
    fun `recording library can be consumed without platform types`() = runTest {
        val item = RecordingLibraryItem(
            historyId = "history-1",
            title = "Recording 1",
            timestampEpochMillis = 1_725_000_000_000,
            durationMillis = 2_500,
            previewText = "Hello",
            status = RecordingLibraryItemStatus.DRAFT,
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
        val port = object : RecordingLibraryPort {
            override suspend fun loadPage(offset: Int, limit: Int) =
                RecordingLibraryPage(listOf(item), hasMore = false)

            override suspend fun loadTranscript(historyId: String) = transcript
        }

        assertEquals(
            RecordingLibraryPage(listOf(item), hasMore = false),
            port.loadPage(offset = 0, limit = 30),
        )
        assertEquals(transcript, port.loadTranscript("history-1"))
    }
}
