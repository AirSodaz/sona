package com.sona.android.app.feature.recording

enum class MicrophonePermissionDecision {
    START_RECORDING,
    REQUEST_PERMISSION,
    SHOW_RATIONALE,
    OPEN_APP_SETTINGS,
}

object MicrophonePermissionPolicy {
    fun decide(
        isGranted: Boolean,
        hasRequestedBefore: Boolean,
        shouldShowRationale: Boolean,
    ): MicrophonePermissionDecision = when {
        isGranted -> MicrophonePermissionDecision.START_RECORDING
        !hasRequestedBefore -> MicrophonePermissionDecision.REQUEST_PERMISSION
        shouldShowRationale -> MicrophonePermissionDecision.SHOW_RATIONALE
        else -> MicrophonePermissionDecision.OPEN_APP_SETTINGS
    }
}
