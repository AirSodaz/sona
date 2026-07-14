package com.sona.android.app.feature.recording

import org.junit.Assert.assertEquals
import org.junit.Test

class MicrophonePermissionPolicyTest {
    @Test
    fun `granted permission proceeds directly to recording`() {
        assertEquals(
            MicrophonePermissionDecision.START_RECORDING,
            MicrophonePermissionPolicy.decide(
                isGranted = true,
                hasRequestedBefore = true,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `first use requests microphone permission`() {
        assertEquals(
            MicrophonePermissionDecision.REQUEST_PERMISSION,
            MicrophonePermissionPolicy.decide(
                isGranted = false,
                hasRequestedBefore = false,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun `denial with rationale shows the in-app rationale`() {
        assertEquals(
            MicrophonePermissionDecision.SHOW_RATIONALE,
            MicrophonePermissionPolicy.decide(
                isGranted = false,
                hasRequestedBefore = true,
                shouldShowRationale = true,
            ),
        )
    }

    @Test
    fun `permanent denial directs the user to app settings`() {
        assertEquals(
            MicrophonePermissionDecision.OPEN_APP_SETTINGS,
            MicrophonePermissionPolicy.decide(
                isGranted = false,
                hasRequestedBefore = true,
                shouldShowRationale = false,
            ),
        )
    }
}
