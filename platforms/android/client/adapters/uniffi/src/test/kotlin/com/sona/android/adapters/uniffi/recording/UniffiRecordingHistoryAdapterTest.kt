package com.sona.android.adapters.uniffi.recording

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
import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.library.RecordingLibraryItemStatus
import com.sona.android.application.library.RecordingLibraryPage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class UniffiRecordingHistoryAdapterTest {
    @Test
    fun `maps recording library pages and transcripts across the UniFFI boundary`() = runTest {
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
                    ),
                ),
                hasMore = true,
            ),
            page,
        )
        assertEquals(listOf(fullSegment()), transcript)
        val request = parseJsonObject(checkNotNull(bindings.queryRequestJson), "query")
        assertEquals("all", request.getValue("scope").jsonObject.getValue("kind").jsonPrimitive.content)
        assertEquals("recording", request.getValue("filterType").jsonPrimitive.content)
        assertEquals("newest", request.getValue("sortOrder").jsonPrimitive.content)
        assertEquals("20", request.getValue("offset").jsonPrimitive.content)
        assertEquals("30", request.getValue("limit").jsonPrimitive.content)
        assertEquals("history-1", bindings.transcriptHistoryId)
    }

    @Test
    fun `maps the history lifecycle across the UniFFI boundary`() = runTest {
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
        val create = parseJsonObject(checkNotNull(bindings.createRequestJson), "create")
        val checkpoint = parseJsonObject(checkNotNull(bindings.updateRequestJson), "checkpoint")
        val checkpointSegment = checkpoint.getValue("segments").jsonArray.single().jsonObject
        val complete = parseJsonObject(checkNotNull(bindings.completeRequestJson), "complete")
        val delete = parseJsonObject(checkNotNull(bindings.deleteRequestJson), "delete")
        assertEquals(
            mapOf(
                "create" to listOf("C:/app-data", "recording-1", "wav", JsonNull, JsonNull),
                "checkpoint" to listOf(
                    "history-1",
                    "segment-1",
                    "token",
                    listOf("he", "llo"),
                    "\u4f60\u597d",
                    "speaker-1",
                    "1",
                ),
                "complete" to listOf("history-1", 2.5, 1),
                "delete" to listOf("history-1"),
            ),
            mapOf(
                "create" to listOf(
                    bindings.createAppDataDir,
                    create.getValue("id").jsonPrimitive.content,
                    create.getValue("audioExtension").jsonPrimitive.content,
                    create.getValue("projectId"),
                    create.getValue("icon"),
                ),
                "checkpoint" to listOf(
                    checkpoint.getValue("historyId").jsonPrimitive.content,
                    checkpointSegment.getValue("id").jsonPrimitive.content,
                    checkpointSegment.getValue("timing").jsonObject
                        .getValue("level").jsonPrimitive.content,
                    checkpointSegment.getValue("tokens").jsonArray.map { it.jsonPrimitive.content },
                    checkpointSegment.getValue("translation").jsonPrimitive.content,
                    checkpointSegment.getValue("speaker").jsonObject
                        .getValue("id").jsonPrimitive.content,
                    checkpointSegment.getValue("speakerAttribution").jsonObject
                        .getValue("candidates").jsonArray.single().jsonObject
                        .getValue("rank").jsonPrimitive.content,
                ),
                "complete" to listOf(
                    summary.historyId,
                    complete.getValue("duration").jsonPrimitive.double,
                    complete.getValue("segments").jsonArray.size,
                ),
                "delete" to delete.getValue("ids").jsonArray.map { it.jsonPrimitive.content },
            ),
        )
    }

    @Test
    fun `rejects malformed binding responses`() {
        val bindings = FakeHistoryBindings().apply { createResponse = "{}" }

        assertThrows(IllegalStateException::class.java) {
            runTest {
                UniffiRecordingHistoryAdapter("C:/app-data", bindings)
                    .createLiveDraft(CreateLiveDraftRequest("recording-1", "wav"))
            }
        }
    }

    @Test
    fun `rejects malformed library responses`() {
        val bindings = FakeHistoryBindings().apply {
            queryResponse = "{\"filteredItems\":[{}],\"hasMore\":false}"
        }

        assertThrows(IllegalStateException::class.java) {
            runTest {
                UniffiRecordingHistoryAdapter("C:/app-data", bindings)
                    .loadPage(offset = 0, limit = 30)
            }
        }
    }

    @Test
    fun `maps an absent persisted transcript to an empty detail`() = runTest {
        val bindings = FakeHistoryBindings().apply { transcriptResponse = "null" }

        assertEquals(
            emptyList<TranscriptSegment>(),
            UniffiRecordingHistoryAdapter("C:/app-data", bindings)
                .loadTranscript("history-1"),
        )
    }

    @Test
    fun `accepts a persisted recording with an empty title`() = runTest {
        val bindings = FakeHistoryBindings().apply {
            queryResponse = queryResponse.replace(
                "\"title\":\"Recording 1\"",
                "\"title\":\"\"",
            )
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

    private class FakeHistoryBindings : UniffiHistoryBindings {
        var createAppDataDir: String? = null
        var createRequestJson: String? = null
        var updateRequestJson: String? = null
        var completeRequestJson: String? = null
        var deleteRequestJson: String? = null
        var queryRequestJson: String? = null
        var transcriptHistoryId: String? = null
        var transcriptFailure: Throwable? = null
        var createResponse =
            "{\"item\":{\"id\":\"history-1\"}," +
                "\"audioAbsolutePath\":\"C:/app-data/history/history-1.wav\"}"
        var queryResponse =
            """{"filteredItems":[{"id":"history-1","timestamp":1725000000000,"duration":2.5,"audioPath":"history-1.wav","audioStatus":"available","transcriptPath":"history-1.json","title":"Recording 1","previewText":"Hello mobile history","icon":null,"type":"recording","searchContent":"","projectId":null,"status":"draft","draftSource":"live_record"}],"searchMatchByItemId":{},"filteredItemCount":1,"hasMore":true,"summary":{"totalItems":1,"totalDuration":2.5,"latestTimestamp":1725000000000,"recordingCount":1,"batchCount":0},"itemCounts":{"inbox":1,"byProjectId":{}}}"""
        var transcriptResponse =
            """[{"id":"segment-1","text":"hello","start":1.25,"end":2.5,"isFinal":true,"timing":{"level":"token","source":"model","units":[{"text":"he","start":1.25,"end":1.5}]},"tokens":["he","llo"],"timestamps":[1.25,1.75],"durations":[0.5,0.75],"translation":"你好","speaker":{"id":"speaker-1","label":"Speaker 1","kind":"known","score":0.8},"speakerAttribution":{"groupId":"group-1","anonymousLabel":"Speaker 1","state":"matched","source":"embedding","confidence":"high","candidates":[{"profileId":"profile-1","profileName":"Alice","score":0.9,"rank":1}]}}]"""

        override suspend fun createLiveDraft(appDataDir: String, requestJson: String): String {
            createAppDataDir = appDataDir
            createRequestJson = requestJson
            return createResponse
        }

        override suspend fun updateTranscript(appDataDir: String, requestJson: String): String {
            updateRequestJson = requestJson
            return "{\"id\":\"history-1\"}"
        }

        override suspend fun completeLiveDraft(appDataDir: String, requestJson: String): String {
            completeRequestJson = requestJson
            return "{\"id\":\"history-1\"}"
        }

        override suspend fun deleteItems(appDataDir: String, requestJson: String): String {
            deleteRequestJson = requestJson
            return "null"
        }

        override suspend fun queryWorkspace(appDataDir: String, requestJson: String): String {
            queryRequestJson = requestJson
            return queryResponse
        }

        override suspend fun loadTranscript(appDataDir: String, historyId: String): String {
            transcriptHistoryId = historyId
            transcriptFailure?.let { throw it }
            return transcriptResponse
        }
    }
}
