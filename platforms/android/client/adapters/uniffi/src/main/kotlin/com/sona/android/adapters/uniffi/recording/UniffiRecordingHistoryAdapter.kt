package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.CompleteLiveDraftRequest
import com.sona.android.application.recording.CreateLiveDraftRequest
import com.sona.android.application.recording.HistoryRecordingSummary
import com.sona.android.application.recording.RecordingDestination
import com.sona.android.application.recording.RecordingDraft
import com.sona.android.application.recording.RecordingHistoryPort
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

class UniffiRecordingHistoryAdapter internal constructor(
    private val appDataDir: String,
    private val bindings: UniffiHistoryBindings,
) : RecordingHistoryPort {
    constructor(appDataDir: String) : this(appDataDir, GeneratedUniffiHistoryBindings)

    init {
        require(appDataDir.isNotBlank()) { "History app data directory must not be blank." }
    }

    override suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft {
        val requestJson = buildJsonObject {
            put("id", request.recordingId)
            put("audioExtension", request.audioExtension)
            put("projectId", JsonNull)
            put("icon", JsonNull)
        }.toString()
        val response = parseJsonObject(
            bindings.createLiveDraft(appDataDir, requestJson),
            "UniFFI history draft",
        )
        val item = response["item"] as? JsonObject
            ?: throw IllegalStateException("UniFFI history draft response is invalid.")
        return RecordingDraft(
            historyId = item.requiredString("id", "UniFFI history draft"),
            destination = RecordingDestination(
                response.requiredString("audioAbsolutePath", "UniFFI history draft"),
            ),
        )
    }

    override suspend fun checkpointTranscript(
        historyId: String,
        segments: List<TranscriptSegment>,
    ) {
        bindings.updateTranscript(
            appDataDir,
            transcriptRequestJson(historyId, segments).toString(),
        )
    }

    override suspend fun completeLiveDraft(
        request: CompleteLiveDraftRequest,
    ): HistoryRecordingSummary {
        val response = parseJsonObject(
            bindings.completeLiveDraft(
                appDataDir,
                buildJsonObject {
                    put("historyId", request.historyId)
                    put("segments", transcriptSegmentsJson(request.segments))
                    put("duration", request.durationMillis / 1_000.0)
                }.toString(),
            ),
            "UniFFI completed history",
        )
        return HistoryRecordingSummary(
            historyId = response.requiredString("id", "UniFFI completed history"),
        )
    }

    override suspend fun deleteDraft(historyId: String) {
        bindings.deleteItems(
            appDataDir,
            buildJsonObject {
                put("ids", JsonArray(listOf(kotlinx.serialization.json.JsonPrimitive(historyId))))
            }.toString(),
        )
    }

    private fun transcriptRequestJson(
        historyId: String,
        segments: List<TranscriptSegment>,
    ): JsonObject = buildJsonObject {
        put("historyId", historyId)
        put("segments", transcriptSegmentsJson(segments))
    }
}

private fun JsonObject.requiredString(key: String, label: String): String =
    this[key]?.jsonPrimitive?.contentOrNull?.takeIf(String::isNotBlank)
        ?: throw IllegalStateException("$label response is invalid.")
