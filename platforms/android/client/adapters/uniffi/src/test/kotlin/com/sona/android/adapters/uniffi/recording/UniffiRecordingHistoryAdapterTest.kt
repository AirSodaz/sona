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
        var createResponse =
            "{\"item\":{\"id\":\"history-1\"}," +
                "\"audioAbsolutePath\":\"C:/app-data/history/history-1.wav\"}"

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
    }
}
