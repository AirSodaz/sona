package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

data class RecordingDestination(
    val value: String,
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

data class AudioInputConfiguration(
    val deviceName: String?,
    val sampleRateHz: Int?,
    val channelCount: Int?,
    val preprocessing: List<String>,
)

sealed interface AudioInputEvent {
    data object MonitoringUnavailable : AudioInputEvent
    data object Active : AudioInputEvent
    data object Silenced : AudioInputEvent
    data class ConfigurationChanged(val configuration: AudioInputConfiguration) : AudioInputEvent
}

enum class MicrophoneCaptureFailureKind {
    AUDIO_READ,
    STORAGE_WRITE,
}

sealed interface MicrophoneCaptureEvent {
    data class Input(val event: AudioInputEvent) : MicrophoneCaptureEvent
    data object StreamingQueueOverflow : MicrophoneCaptureEvent
    data class Fatal(val kind: MicrophoneCaptureFailureKind) : MicrophoneCaptureEvent
}

interface MicrophoneCapturePort {
    suspend fun open(request: MicrophoneCaptureRequest): MicrophoneCaptureSession
}

interface MicrophoneCaptureSession : AutoCloseable {
    val frames: Flow<Pcm16Frame>
    val events: Flow<MicrophoneCaptureEvent>

    suspend fun start()
    suspend fun stop()
    suspend fun finish(): CapturedAudio
}
