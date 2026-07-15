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
import com.sona.android.application.recording.SpeakerAttribution
import com.sona.android.application.recording.SpeakerCandidate
import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import com.sona.android.application.recording.TranscriptTimingUnit
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
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

    override suspend fun loadPage(offset: Int, limit: Int): RecordingLibraryPage {
        require(offset >= 0) { "History offset must not be negative." }
        require(limit in 1..MAX_LIBRARY_PAGE_SIZE) {
            "History limit must be between 1 and $MAX_LIBRARY_PAGE_SIZE."
        }
        val requestJson = buildJsonObject {
            put("scope", buildJsonObject { put("kind", "all") })
            put("query", "")
            put("filterType", "recording")
            put("dateFilter", "all")
            put("sortOrder", "newest")
            put("limit", limit)
            put("offset", offset)
        }.toString()
        val response = parseJsonObject(
            bindings.queryWorkspace(appDataDir, requestJson),
            "UniFFI history query",
        )
        val items = (response["filteredItems"] as? JsonArray)
            ?.map(::parseLibraryItem)
            ?: invalidHistoryResponse()
        return RecordingLibraryPage(
            items = items,
            hasMore = response.requiredBoolean("hasMore", "UniFFI history query"),
        )
    }

    override suspend fun loadTranscript(historyId: String): List<TranscriptSegment> {
        require(historyId.isNotBlank()) { "History ID must not be blank." }
        val transcriptJson = bindings.loadTranscript(appDataDir, historyId)
        val response = try {
            recordingJson.parseToJsonElement(transcriptJson)
        } catch (_: Exception) {
            invalidHistoryResponse()
        }
        return when (response) {
            JsonNull -> emptyList()
            is JsonArray -> response.map(::parseTranscriptSegment)
            else -> invalidHistoryResponse()
        }
    }

    private fun transcriptRequestJson(
        historyId: String,
        segments: List<TranscriptSegment>,
    ): JsonObject = buildJsonObject {
        put("historyId", historyId)
        put("segments", transcriptSegmentsJson(segments))
    }
}

private const val MAX_LIBRARY_PAGE_SIZE = 200

private fun parseLibraryItem(element: JsonElement): RecordingLibraryItem {
    val item = element as? JsonObject ?: invalidHistoryResponse()
    val durationSeconds = item.requiredDouble("duration", "UniFFI history item")
        .coerceAtLeast(0.0)
    val status = when (item.requiredString("status", "UniFFI history item")) {
        "draft" -> RecordingLibraryItemStatus.DRAFT
        "complete" -> RecordingLibraryItemStatus.COMPLETE
        else -> invalidHistoryResponse()
    }
    return RecordingLibraryItem(
        historyId = item.requiredString("id", "UniFFI history item"),
        title = item.requiredContent("title", "UniFFI history item"),
        timestampEpochMillis = item.requiredLong("timestamp", "UniFFI history item"),
        durationMillis = (durationSeconds * 1_000.0).roundToLong(),
        previewText = item.requiredContent("previewText", "UniFFI history item"),
        status = status,
    )
}

private fun parseTranscriptSegment(element: JsonElement): TranscriptSegment {
    val segment = element as? JsonObject ?: invalidHistoryResponse()
    return TranscriptSegment(
        id = segment.requiredString("id", "UniFFI transcript segment"),
        text = segment.requiredContent("text", "UniFFI transcript segment"),
        startSeconds = segment.requiredDouble("start", "UniFFI transcript segment"),
        endSeconds = segment.requiredDouble("end", "UniFFI transcript segment"),
        isFinal = segment.requiredBoolean("isFinal", "UniFFI transcript segment"),
        timing = segment.optionalObject("timing")?.let(::parseTranscriptTiming),
        tokens = segment.optionalArray("tokens")?.map { value ->
            value.requiredPrimitiveContent("UniFFI transcript token")
        },
        timestamps = segment.optionalArray("timestamps")?.map { value ->
            value.requiredPrimitiveContent("UniFFI transcript timestamp")
                .toFloatOrNull() ?: invalidHistoryResponse()
        },
        durations = segment.optionalArray("durations")?.map { value ->
            value.requiredPrimitiveContent("UniFFI transcript duration")
                .toFloatOrNull() ?: invalidHistoryResponse()
        },
        translation = segment.optionalString("translation"),
        speaker = segment.optionalObject("speaker")?.let(::parseSpeaker),
        speakerAttribution = segment.optionalObject("speakerAttribution")
            ?.let(::parseSpeakerAttribution),
    )
}

private fun parseTranscriptTiming(value: JsonObject) =
    TranscriptTiming(
        level = when (value.requiredString("level", "UniFFI transcript timing")) {
            "token" -> TranscriptTimingLevel.TOKEN
            "segment" -> TranscriptTimingLevel.SEGMENT
            else -> invalidHistoryResponse()
        },
        source = when (value.requiredString("source", "UniFFI transcript timing")) {
            "model" -> TranscriptTimingSource.MODEL
            "derived" -> TranscriptTimingSource.DERIVED
            else -> invalidHistoryResponse()
        },
        units = value.requiredArray("units", "UniFFI transcript timing").map { element ->
            val unit = element as? JsonObject ?: invalidHistoryResponse()
            TranscriptTimingUnit(
                text = unit.requiredContent("text", "UniFFI transcript timing unit"),
                startSeconds = unit.requiredDouble("start", "UniFFI transcript timing unit"),
                endSeconds = unit.requiredDouble("end", "UniFFI transcript timing unit"),
            )
        },
    )

private fun parseSpeaker(value: JsonObject) =
    SpeakerTag(
        id = value.requiredString("id", "UniFFI transcript speaker"),
        label = value.requiredString("label", "UniFFI transcript speaker"),
        kind = value.requiredString("kind", "UniFFI transcript speaker"),
        score = value.optionalNumber("score")?.toFloat(),
    )

private fun parseSpeakerAttribution(value: JsonObject) =
    SpeakerAttribution(
        groupId = value.requiredString("groupId", "UniFFI speaker attribution"),
        anonymousLabel = value.requiredString("anonymousLabel", "UniFFI speaker attribution"),
        state = value.requiredString("state", "UniFFI speaker attribution"),
        source = value.requiredString("source", "UniFFI speaker attribution"),
        confidence = value.requiredString("confidence", "UniFFI speaker attribution"),
        candidates = value.requiredArray("candidates", "UniFFI speaker attribution")
            .map { element ->
                val candidate = element as? JsonObject ?: invalidHistoryResponse()
                SpeakerCandidate(
                    profileId = candidate.requiredString(
                        "profileId",
                        "UniFFI speaker candidate",
                    ),
                    profileName = candidate.requiredString(
                        "profileName",
                        "UniFFI speaker candidate",
                    ),
                    score = candidate.requiredDouble("score", "UniFFI speaker candidate").toFloat(),
                    rank = candidate.requiredContent("rank", "UniFFI speaker candidate")
                        .toULongOrNull() ?: invalidHistoryResponse(),
                )
            },
    )

private fun JsonObject.requiredString(key: String, label: String): String =
    this[key]?.jsonPrimitive?.contentOrNull?.takeIf(String::isNotBlank)
        ?: throw IllegalStateException("$label response is invalid.")

private fun JsonObject.requiredContent(key: String, label: String): String =
    this[key]?.let { element ->
        (element as? JsonPrimitive)?.contentOrNull
    } ?: throw IllegalStateException("$label response is invalid.")

private fun JsonObject.requiredBoolean(key: String, label: String): Boolean =
    requiredContent(key, label).toBooleanStrictOrNull()
        ?: throw IllegalStateException("$label response is invalid.")

private fun JsonObject.requiredDouble(key: String, label: String): Double =
    requiredContent(key, label).toDoubleOrNull()
        ?: throw IllegalStateException("$label response is invalid.")

private fun JsonObject.requiredLong(key: String, label: String): Long =
    requiredContent(key, label).toLongOrNull()
        ?: throw IllegalStateException("$label response is invalid.")

private fun JsonObject.requiredArray(key: String, label: String): JsonArray =
    this[key] as? JsonArray ?: throw IllegalStateException("$label response is invalid.")

private fun JsonObject.optionalArray(key: String): JsonArray? = when (val value = this[key]) {
    null, JsonNull -> null
    is JsonArray -> value
    else -> invalidHistoryResponse()
}

private fun JsonObject.optionalObject(key: String): JsonObject? = when (val value = this[key]) {
    null, JsonNull -> null
    is JsonObject -> value
    else -> invalidHistoryResponse()
}

private fun JsonObject.optionalString(key: String): String? = when (val value = this[key]) {
    null, JsonNull -> null
    is JsonPrimitive -> value.contentOrNull ?: invalidHistoryResponse()
    else -> invalidHistoryResponse()
}

private fun JsonObject.optionalNumber(key: String): Double? = when (val value = this[key]) {
    null, JsonNull -> null
    is JsonPrimitive -> value.contentOrNull?.toDoubleOrNull() ?: invalidHistoryResponse()
    else -> invalidHistoryResponse()
}

private fun JsonElement.requiredPrimitiveContent(label: String): String =
    (this as? JsonPrimitive)?.contentOrNull
        ?: throw IllegalStateException("$label response is invalid.")

private fun invalidHistoryResponse(): Nothing =
    throw IllegalStateException("UniFFI history response is invalid.")
