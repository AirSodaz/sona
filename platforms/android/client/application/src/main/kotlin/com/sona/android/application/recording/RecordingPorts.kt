package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

class StreamingCredential(
    val apiKey: String,
) {
    override fun equals(other: Any?): Boolean =
        other is StreamingCredential && apiKey == other.apiKey

    override fun hashCode(): Int = apiKey.hashCode()

    override fun toString(): String = "StreamingCredential(apiKey=<redacted>)"
}

data class StreamingProviderProfile(
    val providerId: String,
    val profileId: String,
    val streamingEndpoint: String,
    val streamingResourceId: String,
)

data class RecordingDestination(
    val value: String,
)

data class CreateLiveDraftRequest(
    val recordingId: String,
    val audioExtension: String,
)

data class RecordingDraft(
    val historyId: String,
    val destination: RecordingDestination,
)

data class CompleteLiveDraftRequest(
    val historyId: String,
    val segments: List<TranscriptSegment>,
    val durationMillis: Long,
)

data class HistoryRecordingSummary(
    val historyId: String,
)

data class MicrophoneCaptureRequest(
    val recordingId: String,
    val destination: RecordingDestination,
    val sampleRateHz: Int,
    val channelCount: Int,
    val bitsPerSample: Int,
)

data class Pcm16Frame(
    val bytes: ByteArray,
)

data class CapturedAudio(
    val durationMillis: Long,
    val bytesWritten: Long,
)

sealed interface AudioInputEvent {
    data object Active : AudioInputEvent
    data object Silenced : AudioInputEvent

    data class DeviceChanged(
        val deviceName: String?,
    ) : AudioInputEvent
}

data class StreamingTranscriptionRequest(
    val recordingId: String,
    val credential: StreamingCredential,
    val profile: StreamingProviderProfile,
    val language: String,
    val enableItn: Boolean,
)

sealed interface StreamingTranscriptionEvent {
    data class Transcript(
        val update: TranscriptUpdate,
    ) : StreamingTranscriptionEvent
}

interface StreamingCredentialRepository {
    suspend fun load(): StreamingCredential?
    suspend fun save(credential: StreamingCredential)
    suspend fun clear()
}

fun interface StreamingProviderCatalogPort {
    suspend fun loadVolcengineStreamingProfile(): StreamingProviderProfile
}

interface MicrophoneCapturePort {
    suspend fun open(request: MicrophoneCaptureRequest): MicrophoneCaptureSession
}

interface MicrophoneCaptureSession : AutoCloseable {
    val frames: Flow<Pcm16Frame>
    val inputEvents: Flow<AudioInputEvent>

    suspend fun start()
    suspend fun stop()
    suspend fun finish(): CapturedAudio
}

interface StreamingTranscriptionPort {
    suspend fun open(request: StreamingTranscriptionRequest): StreamingTranscriptionSession
}

interface StreamingTranscriptionSession : AutoCloseable {
    val events: Flow<StreamingTranscriptionEvent>

    suspend fun start()
    suspend fun feed(frame: Pcm16Frame)
    suspend fun flush()
    suspend fun stop()
}

interface RecordingHistoryPort {
    suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft

    suspend fun checkpointTranscript(
        historyId: String,
        segments: List<TranscriptSegment>,
    )

    suspend fun completeLiveDraft(request: CompleteLiveDraftRequest): HistoryRecordingSummary
    suspend fun deleteDraft(historyId: String)
}

fun interface WallClockPort {
    fun nowEpochMillis(): Long
}

fun interface MonotonicClockPort {
    fun elapsedRealtimeMillis(): Long
}

fun interface RecordingIdPort {
    fun nextRecordingId(): String
}
