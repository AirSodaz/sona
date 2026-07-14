package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.SpeakerAttribution
import com.sona.android.application.recording.SpeakerCandidate
import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.StreamingTranscriptionRequest
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingUnit
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

internal val recordingJson = Json {
    ignoreUnknownKeys = true
}

internal fun buildStreamingConfigJson(request: StreamingTranscriptionRequest): String =
    buildJsonObject {
        put("apiKey", request.credential.apiKey)
        put("streamingEndpoint", request.profile.streamingEndpoint)
        put("streamingResourceId", request.profile.streamingResourceId)
    }.toString()

internal fun buildStreamingRequestJson(
    request: StreamingTranscriptionRequest,
    provider: UniffiOnlineProviderRequest,
): String = buildJsonObject {
    put("mode", "streaming")
    put("language", request.language)
    put("enableItn", request.enableItn)
    put("normalizationOptions", buildJsonObject {
        put("enableTimeline", false)
    })
    put("postprocessOptions", buildJsonObject {
        put("textReplacementSets", buildJsonArray {})
        put("dropFinalDotSegments", true)
    })
    put("hotwords", JsonNull)
    put("speakerProcessing", JsonNull)
    put("engine", "online")
    put("onlineProvider", buildJsonObject {
        put("providerId", provider.providerId)
        put("profileId", provider.profileId)
        put("config", recordingJson.parseToJsonElement(provider.configJson))
    })
}.toString()

internal fun transcriptSegmentsJson(segments: List<TranscriptSegment>): JsonArray =
    JsonArray(segments.map(::transcriptSegmentJson))

internal fun parseJsonObject(value: String, label: String): JsonObject = try {
    recordingJson.parseToJsonElement(value) as JsonObject
} catch (_: Exception) {
    throw IllegalStateException("$label response is invalid.")
}

private fun transcriptSegmentJson(segment: TranscriptSegment): JsonObject = buildJsonObject {
    put("id", segment.id)
    put("text", segment.text)
    put("start", segment.startSeconds)
    put("end", segment.endSeconds)
    put("isFinal", segment.isFinal)
    segment.timing?.let { put("timing", transcriptTimingJson(it)) }
    segment.tokens?.let { values -> put("tokens", stringArray(values)) }
    segment.timestamps?.let { values -> put("timestamps", floatArray(values)) }
    segment.durations?.let { values -> put("durations", floatArray(values)) }
    segment.translation?.let { put("translation", it) }
    segment.speaker?.let { put("speaker", speakerJson(it)) }
    segment.speakerAttribution?.let { put("speakerAttribution", speakerAttributionJson(it)) }
}

private fun transcriptTimingJson(timing: TranscriptTiming): JsonObject = buildJsonObject {
    put("level", timing.level.name.lowercase())
    put("source", timing.source.name.lowercase())
    put("units", JsonArray(timing.units.map(::transcriptTimingUnitJson)))
}

private fun transcriptTimingUnitJson(unit: TranscriptTimingUnit): JsonObject = buildJsonObject {
    put("text", unit.text)
    put("start", unit.startSeconds)
    put("end", unit.endSeconds)
}

private fun speakerJson(speaker: SpeakerTag): JsonObject = buildJsonObject {
    put("id", speaker.id)
    put("label", speaker.label)
    put("kind", speaker.kind)
    speaker.score?.let { put("score", it) }
}

private fun speakerAttributionJson(attribution: SpeakerAttribution): JsonObject = buildJsonObject {
    put("groupId", attribution.groupId)
    put("anonymousLabel", attribution.anonymousLabel)
    put("state", attribution.state)
    put("source", attribution.source)
    put("confidence", attribution.confidence)
    put("candidates", JsonArray(attribution.candidates.map(::speakerCandidateJson)))
}

private fun speakerCandidateJson(candidate: SpeakerCandidate): JsonObject = buildJsonObject {
    put("profileId", candidate.profileId)
    put("profileName", candidate.profileName)
    put("score", candidate.score)
    put("rank", unsignedJsonNumber(candidate.rank))
}

private fun stringArray(values: List<String>): JsonArray =
    JsonArray(values.map(::JsonPrimitive))

private fun floatArray(values: List<Float>): JsonArray =
    JsonArray(values.map(::JsonPrimitive))

private fun unsignedJsonNumber(value: ULong): JsonElement =
    recordingJson.parseToJsonElement(value.toString())
