package com.sona.android.app.feature.recording

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat

enum class MicrophonePermissionDecision {
    START_RECORDING,
    REQUEST_PERMISSION,
    SHOW_RATIONALE,
    OPEN_APP_SETTINGS,
}

object MicrophonePermissionPolicy {
    fun isGranted(context: Context): Boolean = ContextCompat.checkSelfPermission(
        context,
        Manifest.permission.RECORD_AUDIO,
    ) == PackageManager.PERMISSION_GRANTED

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
