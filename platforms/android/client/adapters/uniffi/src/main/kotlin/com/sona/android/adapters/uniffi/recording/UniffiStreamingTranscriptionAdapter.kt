package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.Pcm16Frame
import com.sona.android.application.recording.SpeakerAttribution
import com.sona.android.application.recording.SpeakerCandidate
import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.StreamingTranscriptionEvent
import com.sona.android.application.recording.StreamingTranscriptionPort
import com.sona.android.application.recording.StreamingTranscriptionRequest
import com.sona.android.application.recording.StreamingTranscriptionSession
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import com.sona.android.application.recording.TranscriptTimingUnit
import com.sona.android.application.recording.TranscriptUpdate
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import uniffi.sona_uniffi_bind.FfiAsrInferenceMetric
import uniffi.sona_uniffi_bind.FfiAsrModelLoadMetric
import uniffi.sona_uniffi_bind.FfiAsrStreamingErrorEvent
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.FfiSpeakerAttribution
import uniffi.sona_uniffi_bind.FfiSpeakerCandidate
import uniffi.sona_uniffi_bind.FfiSpeakerTag
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.FfiTranscriptTiming
import uniffi.sona_uniffi_bind.FfiTranscriptTimingLevel
import uniffi.sona_uniffi_bind.FfiTranscriptTimingSource
import uniffi.sona_uniffi_bind.FfiTranscriptTimingUnit
import uniffi.sona_uniffi_bind.FfiTranscriptUpdate

class UniffiStreamingTranscriptionAdapter internal constructor(
    private val bindings: UniffiStreamingBindings,
) : StreamingTranscriptionPort {
    constructor() : this(GeneratedUniffiStreamingBindings)

    override suspend fun open(
        request: StreamingTranscriptionRequest,
    ): StreamingTranscriptionSession {
        val configJson = buildStreamingConfigJson(request)
        val providerRequest = bindings.resolveProviderRequest(
            providerId = request.profile.providerId,
            profileId = request.profile.profileId,
            configJson = configJson,
        )
        val requestJson = buildStreamingRequestJson(request, providerRequest)
        val events = Channel<StreamingTranscriptionEvent>(Channel.UNLIMITED)
        val observer = StreamingObserver(request.recordingId, events)
        val handle = try {
            bindings.createSession(request.recordingId, requestJson, observer)
        } catch (error: Throwable) {
            events.close()
            throw error
        }
        return UniffiStreamingTranscriptionSession(handle, events)
    }
}

private class UniffiStreamingTranscriptionSession(
    private val handle: UniffiStreamingSessionHandle,
    private val eventChannel: Channel<StreamingTranscriptionEvent>,
) : StreamingTranscriptionSession {
    private val stopped = AtomicBoolean()
    private val closed = AtomicBoolean()

    override val events: Flow<StreamingTranscriptionEvent> = eventChannel.receiveAsFlow()

    override suspend fun start() {
        check(!closed.get()) { "Streaming transcription session is closed." }
        handle.start()
    }

    override suspend fun feed(frame: Pcm16Frame) {
        check(!closed.get()) { "Streaming transcription session is closed." }
        handle.feedAudioChunk(frame.bytes)
    }

    override suspend fun flush() {
        check(!closed.get()) { "Streaming transcription session is closed." }
        handle.flush()
    }

    override suspend fun stop() {
        if (!stopped.compareAndSet(false, true)) {
            eventChannel.close()
            return
        }
        try {
            handle.stop()
        } finally {
            eventChannel.close()
        }
    }

    override fun close() {
        stopped.set(true)
        eventChannel.close()
        if (closed.compareAndSet(false, true)) {
            handle.close()
        }
    }
}

private class StreamingObserver(
    private val recordingId: String,
    private val events: Channel<StreamingTranscriptionEvent>,
) : FfiAsrStreamingObserver {
    private val terminal = AtomicBoolean()

    override fun onTranscriptUpdate(event: FfiAsrTranscriptUpdateEvent) {
        if (event.instanceId != recordingId) {
            fail(
                code = "UNEXPECTED_STREAM_INSTANCE",
                message = "Streaming callback instance did not match the active recording.",
            )
            return
        }
        val update = try {
            event.update.toApplication()
        } catch (_: Exception) {
            fail(
                code = "INVALID_STREAM_EVENT",
                message = "Streaming callback payload was invalid.",
            )
            return
        }
        if (!terminal.get()) {
            events.trySend(StreamingTranscriptionEvent.Transcript(update))
        }
    }

    override fun onModelLoad(metric: FfiAsrModelLoadMetric) = Unit

    override fun onLiveInference(metric: FfiAsrInferenceMetric) = Unit

    override fun onStreamingError(event: FfiAsrStreamingErrorEvent) {
        if (event.instanceId != recordingId) {
            fail(
                code = "UNEXPECTED_STREAM_INSTANCE",
                message = "Streaming callback instance did not match the active recording.",
            )
        } else {
            fail(event.code, event.message)
        }
    }

    private fun fail(code: String, message: String) {
        if (terminal.compareAndSet(false, true)) {
            events.trySend(StreamingTranscriptionEvent.Failure(code, message))
            events.close()
        }
    }
}

private fun FfiTranscriptUpdate.toApplication(): TranscriptUpdate = TranscriptUpdate(
    removeIds = removeIds,
    upsertSegments = upsertSegments.map(FfiTranscriptSegment::toApplication),
)

private fun FfiTranscriptSegment.toApplication(): TranscriptSegment = TranscriptSegment(
    id = id,
    text = text,
    startSeconds = start,
    endSeconds = end,
    isFinal = isFinal,
    timing = timing?.toApplication(),
    tokens = tokens,
    timestamps = timestamps,
    durations = durations,
    translation = translation,
    speaker = speaker?.toApplication(),
    speakerAttribution = speakerAttribution?.toApplication(),
)

private fun FfiTranscriptTiming.toApplication(): TranscriptTiming = TranscriptTiming(
    level = when (level) {
        FfiTranscriptTimingLevel.TOKEN -> TranscriptTimingLevel.TOKEN
        FfiTranscriptTimingLevel.SEGMENT -> TranscriptTimingLevel.SEGMENT
    },
    source = when (source) {
        FfiTranscriptTimingSource.MODEL -> TranscriptTimingSource.MODEL
        FfiTranscriptTimingSource.DERIVED -> TranscriptTimingSource.DERIVED
    },
    units = units.map(FfiTranscriptTimingUnit::toApplication),
)

private fun FfiTranscriptTimingUnit.toApplication(): TranscriptTimingUnit = TranscriptTimingUnit(
    text = text,
    startSeconds = start,
    endSeconds = end,
)

private fun FfiSpeakerTag.toApplication(): SpeakerTag = SpeakerTag(
    id = id,
    label = label,
    kind = kind,
    score = score,
)

private fun FfiSpeakerCandidate.toApplication(): SpeakerCandidate = SpeakerCandidate(
    profileId = profileId,
    profileName = profileName,
    score = score,
    rank = rank,
)

private fun FfiSpeakerAttribution.toApplication(): SpeakerAttribution = SpeakerAttribution(
    groupId = groupId,
    anonymousLabel = anonymousLabel,
    state = state,
    source = source,
    confidence = confidence,
    candidates = candidates.map(FfiSpeakerCandidate::toApplication),
)
