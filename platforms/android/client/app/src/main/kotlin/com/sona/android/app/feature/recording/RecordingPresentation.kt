package com.sona.android.app.feature.recording

import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.RecordingFailureCategory
import java.util.Locale

enum class RecordingStatusCategory {
    IDLE,
    NEEDS_CONFIGURATION,
    PREPARING,
    RECORDING,
    STOPPING,
    COMPLETED,
    COMPLETED_WITH_WARNING,
    INVALID_CONFIGURATION_FAILURE,
    STARTUP_FAILURE,
    AUDIO_FAILURE,
    STREAMING_FAILURE,
    PERSISTENCE_FAILURE,
}

data class RecordingPresentation(
    val isStartAvailable: Boolean,
    val isStopAvailable: Boolean,
    val statusCategory: RecordingStatusCategory,
)

fun formatRecordingTimer(elapsedMillis: Long): String {
    val elapsedSeconds = elapsedMillis.coerceAtLeast(0) / 1_000
    val minutes = elapsedSeconds / 60
    val seconds = elapsedSeconds % 60
    return String.format(Locale.ROOT, "%02d:%02d", minutes, seconds)
}

fun LiveRecordingState.toRecordingPresentation(): RecordingPresentation = when (this) {
    LiveRecordingState.Idle -> RecordingPresentation(
        isStartAvailable = true,
        isStopAvailable = false,
        statusCategory = RecordingStatusCategory.IDLE,
    )

    LiveRecordingState.NeedsConfiguration -> RecordingPresentation(
        isStartAvailable = true,
        isStopAvailable = false,
        statusCategory = RecordingStatusCategory.NEEDS_CONFIGURATION,
    )

    is LiveRecordingState.Preparing -> RecordingPresentation(
        isStartAvailable = false,
        isStopAvailable = false,
        statusCategory = RecordingStatusCategory.PREPARING,
    )

    is LiveRecordingState.Recording -> RecordingPresentation(
        isStartAvailable = false,
        isStopAvailable = true,
        statusCategory = RecordingStatusCategory.RECORDING,
    )

    is LiveRecordingState.Stopping -> RecordingPresentation(
        isStartAvailable = false,
        isStopAvailable = false,
        statusCategory = RecordingStatusCategory.STOPPING,
    )

    is LiveRecordingState.Completed -> RecordingPresentation(
        isStartAvailable = true,
        isStopAvailable = false,
        statusCategory = if (warning == null) {
            RecordingStatusCategory.COMPLETED
        } else {
            RecordingStatusCategory.COMPLETED_WITH_WARNING
        },
    )

    is LiveRecordingState.Failed -> RecordingPresentation(
        isStartAvailable = true,
        isStopAvailable = false,
        statusCategory = when (failure.category) {
            RecordingFailureCategory.INVALID_CONFIGURATION ->
                RecordingStatusCategory.INVALID_CONFIGURATION_FAILURE
            RecordingFailureCategory.STARTUP -> RecordingStatusCategory.STARTUP_FAILURE
            RecordingFailureCategory.AUDIO -> RecordingStatusCategory.AUDIO_FAILURE
            RecordingFailureCategory.STREAMING -> RecordingStatusCategory.STREAMING_FAILURE
            RecordingFailureCategory.PERSISTENCE -> RecordingStatusCategory.PERSISTENCE_FAILURE
        },
    )
}
