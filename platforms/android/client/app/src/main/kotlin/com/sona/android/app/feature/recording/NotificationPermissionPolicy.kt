package com.sona.android.app.feature.recording

internal enum class NotificationPermissionDecision {
    START_RECORDING,
    REQUEST_PERMISSION,
}

internal object NotificationPermissionPolicy {
    fun decide(
        requiresRuntimePermission: Boolean,
        isGranted: Boolean,
        hasRequestedBefore: Boolean,
    ): NotificationPermissionDecision = when {
        !requiresRuntimePermission || isGranted || hasRequestedBefore ->
            NotificationPermissionDecision.START_RECORDING
        else -> NotificationPermissionDecision.REQUEST_PERMISSION
    }
}
