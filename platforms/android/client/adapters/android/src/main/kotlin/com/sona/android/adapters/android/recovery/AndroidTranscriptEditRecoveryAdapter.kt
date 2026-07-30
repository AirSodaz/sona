package com.sona.android.adapters.android.recovery

import com.sona.android.application.recording.SpeakerAttribution
import com.sona.android.application.recording.SpeakerCandidate
import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import com.sona.android.application.recording.TranscriptTimingUnit
import com.sona.android.application.recovery.RecoveryItemInput
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySource
import com.sona.android.application.recovery.RecoveryStage
import com.sona.android.application.recovery.TranscriptEditDraft
import com.sona.android.application.recovery.TranscriptEditRecoveryPort
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.double
import kotlinx.serialization.json.float
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.put

class AndroidTranscriptEditRecoveryAdapter(
    private val coordinator: RecoveryCoordinator,
) : TranscriptEditRecoveryPort {
    override suspend fun load(historyId: String): TranscriptEditDraft? {
        require(historyId.isNotBlank()) { "History ID must not be blank." }
        return coordinator.load().items
            .firstOrNull {
                it.source == RecoverySource.TRANSCRIPT_EDIT &&
                    it.resolution == RecoveryResolution.PENDING &&
                    it.historyId == historyId
            }
            ?.payload
            ?.let(::decodeDraft)
    }

    override suspend fun save(draft: TranscriptEditDraft) {
        require(draft.historyId.isNotBlank()) { "History ID must not be blank." }
        require(draft.recoveryId.isNotBlank()) { "Recovery ID must not be blank." }
        require(draft.editSessionId.isNotBlank()) { "Edit session ID must not be blank." }
        coordinator.upsert(
            RecoveryItemInput(
                id = draft.recoveryId,
                filename = draft.historyTitle.ifBlank { "Transcript edit" },
                filePath = "",
                historyId = draft.historyId,
                historyTitle = draft.historyTitle,
                stage = RecoveryStage.SAVING,
                payload = encodeDraft(draft),
                source = RecoverySource.TRANSCRIPT_EDIT,
                hasSourceFile = true,
                canResume = true,
            ),
        )
    }

    override suspend fun discard(historyId: String) {
        val item = coordinator.load().items.firstOrNull {
            it.source == RecoverySource.TRANSCRIPT_EDIT &&
                it.resolution == RecoveryResolution.PENDING &&
                it.historyId == historyId
        } ?: return
        coordinator.resolve(item.id)
    }
}

internal fun encodeDraft(draft: TranscriptEditDraft): String = buildJsonObject {
    put("androidTranscriptEditV1", buildJsonObject {
        put("recoveryId", draft.recoveryId)
        put("editSessionId", draft.editSessionId)
        put("historyId", draft.historyId)
        put("historyTitle", draft.historyTitle)
        put("baseSegments", draft.baseSegments.toJson())
        put("draftSegments", draft.draftSegments.toJson())
    })
}.toString()

internal fun decodeDraft(payload: String): TranscriptEditDraft? = runCatching {
    val root = Json.parseToJsonElement(payload).jsonObject["androidTranscriptEditV1"]?.jsonObject
        ?: return@runCatching null
    TranscriptEditDraft(
        recoveryId = root.getValue("recoveryId").jsonPrimitive.content,
        editSessionId = root.getValue("editSessionId").jsonPrimitive.content,
        historyId = root.getValue("historyId").jsonPrimitive.content,
        historyTitle = root["historyTitle"]?.jsonPrimitive?.contentOrNull.orEmpty(),
        baseSegments = root.getValue("baseSegments").jsonArray.map(::segmentFromJson),
        draftSegments = root.getValue("draftSegments").jsonArray.map(::segmentFromJson),
    )
}.getOrNull()

private fun List<TranscriptSegment>.toJson() = buildJsonArray { forEach { add(it.toJson()) } }

private fun TranscriptSegment.toJson() = buildJsonObject {
    put("id", id)
    put("text", text)
    put("startSeconds", startSeconds)
    put("endSeconds", endSeconds)
    put("isFinal", isFinal)
    timing?.let { value -> put("timing", value.toJson()) }
    tokens?.let { values -> put("tokens", JsonArray(values.map(::JsonPrimitive))) }
    timestamps?.let { values -> put("timestamps", JsonArray(values.map(::JsonPrimitive))) }
    durations?.let { values -> put("durations", JsonArray(values.map(::JsonPrimitive))) }
    translation?.let { put("translation", it) }
    speaker?.let { put("speaker", it.toJson()) }
    speakerAttribution?.let { put("speakerAttribution", it.toJson()) }
}

private fun TranscriptTiming.toJson() = buildJsonObject {
    put("level", level.name)
    put("source", source.name)
    put("units", buildJsonArray {
        units.forEach { unit -> add(buildJsonObject {
            put("text", unit.text)
            put("startSeconds", unit.startSeconds)
            put("endSeconds", unit.endSeconds)
        }) }
    })
}

private fun SpeakerTag.toJson() = buildJsonObject {
    put("id", id)
    put("label", label)
    put("kind", kind)
    score?.let { put("score", it) }
}

private fun SpeakerAttribution.toJson() = buildJsonObject {
    put("groupId", groupId)
    put("anonymousLabel", anonymousLabel)
    put("state", state)
    put("source", source)
    put("confidence", confidence)
    put("candidates", buildJsonArray {
        candidates.forEach { candidate -> add(buildJsonObject {
            put("profileId", candidate.profileId)
            put("profileName", candidate.profileName)
            put("score", candidate.score)
            put("rank", candidate.rank.toString())
        }) }
    })
}

private fun segmentFromJson(value: kotlinx.serialization.json.JsonElement): TranscriptSegment {
    val item = value.jsonObject
    return TranscriptSegment(
        id = item.getValue("id").jsonPrimitive.content,
        text = item.getValue("text").jsonPrimitive.content,
        startSeconds = item.getValue("startSeconds").jsonPrimitive.double,
        endSeconds = item.getValue("endSeconds").jsonPrimitive.double,
        isFinal = item.getValue("isFinal").jsonPrimitive.boolean,
        timing = item["timing"]?.jsonObject?.let(::timingFromJson),
        tokens = item["tokens"]?.jsonArray?.map { it.jsonPrimitive.content },
        timestamps = item["timestamps"]?.jsonArray?.map { it.jsonPrimitive.float },
        durations = item["durations"]?.jsonArray?.map { it.jsonPrimitive.float },
        translation = item["translation"]?.jsonPrimitive?.contentOrNull,
        speaker = item["speaker"]?.jsonObject?.let(::speakerFromJson),
        speakerAttribution = item["speakerAttribution"]?.jsonObject?.let(::attributionFromJson),
    )
}

private fun timingFromJson(value: JsonObject) = TranscriptTiming(
    level = TranscriptTimingLevel.valueOf(value.getValue("level").jsonPrimitive.content),
    source = TranscriptTimingSource.valueOf(value.getValue("source").jsonPrimitive.content),
    units = value.getValue("units").jsonArray.map { unitValue ->
        val unit = unitValue.jsonObject
        TranscriptTimingUnit(
            text = unit.getValue("text").jsonPrimitive.content,
            startSeconds = unit.getValue("startSeconds").jsonPrimitive.double,
            endSeconds = unit.getValue("endSeconds").jsonPrimitive.double,
        )
    },
)

private fun speakerFromJson(value: JsonObject) = SpeakerTag(
    id = value.getValue("id").jsonPrimitive.content,
    label = value.getValue("label").jsonPrimitive.content,
    kind = value.getValue("kind").jsonPrimitive.content,
    score = value["score"]?.jsonPrimitive?.float,
)

private fun attributionFromJson(value: JsonObject) = SpeakerAttribution(
    groupId = value.getValue("groupId").jsonPrimitive.content,
    anonymousLabel = value.getValue("anonymousLabel").jsonPrimitive.content,
    state = value.getValue("state").jsonPrimitive.content,
    source = value.getValue("source").jsonPrimitive.content,
    confidence = value.getValue("confidence").jsonPrimitive.content,
    candidates = value.getValue("candidates").jsonArray.map { candidateValue ->
        val candidate = candidateValue.jsonObject
        SpeakerCandidate(
            profileId = candidate.getValue("profileId").jsonPrimitive.content,
            profileName = candidate.getValue("profileName").jsonPrimitive.content,
            score = candidate.getValue("score").jsonPrimitive.float,
            rank = candidate.getValue("rank").jsonPrimitive.content.toULong(),
        )
    },
)
