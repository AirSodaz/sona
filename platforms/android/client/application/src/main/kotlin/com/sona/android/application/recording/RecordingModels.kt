package com.sona.android.application.recording

enum class TranscriptTimingLevel {
    TOKEN,
    SEGMENT,
}

enum class TranscriptTimingSource {
    MODEL,
    DERIVED,
}

data class TranscriptTimingUnit(
    val text: String,
    val startSeconds: Double,
    val endSeconds: Double,
)

data class TranscriptTiming(
    val level: TranscriptTimingLevel,
    val source: TranscriptTimingSource,
    val units: List<TranscriptTimingUnit>,
)

data class SpeakerTag(
    val id: String,
    val label: String,
    val kind: String,
    val score: Float? = null,
)

data class SpeakerCandidate(
    val profileId: String,
    val profileName: String,
    val score: Float,
    val rank: ULong,
)

data class SpeakerAttribution(
    val groupId: String,
    val anonymousLabel: String,
    val state: String,
    val source: String,
    val confidence: String,
    val candidates: List<SpeakerCandidate>,
)

data class TranscriptSegment(
    val id: String,
    val text: String,
    val startSeconds: Double,
    val endSeconds: Double,
    val isFinal: Boolean,
    val timing: TranscriptTiming? = null,
    val tokens: List<String>? = null,
    val timestamps: List<Float>? = null,
    val durations: List<Float>? = null,
    val translation: String? = null,
    val speaker: SpeakerTag? = null,
    val speakerAttribution: SpeakerAttribution? = null,
)

data class TranscriptUpdate(
    val removeIds: List<String>,
    val upsertSegments: List<TranscriptSegment>,
)

enum class RecordingFailureCategory {
    INVALID_CONFIGURATION,
    STARTUP,
    AUDIO,
    STREAMING,
    PERSISTENCE,
}

data class RecordingFailure(
    val category: RecordingFailureCategory,
    val message: String,
)

sealed interface StreamingStatus {
    data object Connected : StreamingStatus

    data class AudioOnly(
        val failure: RecordingFailure,
    ) : StreamingStatus
}

sealed interface AudioInputStatus {
    data object Active : AudioInputStatus
    data object Silenced : AudioInputStatus
    data object MonitoringUnavailable : AudioInputStatus

    data class DeviceChanged(
        val deviceName: String?,
    ) : AudioInputStatus
}

sealed interface LiveRecordingState {
    data object Idle : LiveRecordingState
    data object NeedsConfiguration : LiveRecordingState

    data class Preparing(
        val recordingId: String,
    ) : LiveRecordingState

    data class Recording(
        val recordingId: String,
        val elapsedMillis: Long,
        val segments: List<TranscriptSegment>,
        val streamingStatus: StreamingStatus,
        val inputStatus: AudioInputStatus,
    ) : LiveRecordingState

    data class Stopping(
        val recordingId: String,
    ) : LiveRecordingState

    data class Completed(
        val historyId: String,
        val warning: RecordingFailure? = null,
    ) : LiveRecordingState

    data class Failed(
        val failure: RecordingFailure,
    ) : LiveRecordingState
}
