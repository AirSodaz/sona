package com.sona.android.app.feature.recording

import org.junit.Assert.assertEquals
import org.junit.Test

class MicrophonePermissionPolicyTest {
    @Test
    fun `maps permission state to the next recording action`() {
        fun decide(granted: Boolean, requested: Boolean, rationale: Boolean) =
            MicrophonePermissionPolicy.decide(granted, requested, rationale)

        assertEquals(MicrophonePermissionDecision.START_RECORDING, decide(true, true, false))
        assertEquals(MicrophonePermissionDecision.REQUEST_PERMISSION, decide(false, false, false))
        assertEquals(MicrophonePermissionDecision.SHOW_RATIONALE, decide(false, true, true))
        assertEquals(MicrophonePermissionDecision.OPEN_APP_SETTINGS, decide(false, true, false))
    }
}
