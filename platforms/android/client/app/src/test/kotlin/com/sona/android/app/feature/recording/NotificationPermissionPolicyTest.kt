package com.sona.android.app.feature.recording

import org.junit.Assert.assertEquals
import org.junit.Test

class NotificationPermissionPolicyTest {
    @Test
    fun `pre Android 13 starts without requesting notification permission`() {
        assertEquals(
            NotificationPermissionDecision.START_RECORDING,
            NotificationPermissionPolicy.decide(
                requiresRuntimePermission = false,
                isGranted = false,
                hasRequestedBefore = false,
            ),
        )
    }

    @Test
    fun `first Android 13 start requests notification permission`() {
        assertEquals(
            NotificationPermissionDecision.REQUEST_PERMISSION,
            NotificationPermissionPolicy.decide(
                requiresRuntimePermission = true,
                isGranted = false,
                hasRequestedBefore = false,
            ),
        )
    }

    @Test
    fun `denied notification permission does not block a later recording start`() {
        assertEquals(
            NotificationPermissionDecision.START_RECORDING,
            NotificationPermissionPolicy.decide(
                requiresRuntimePermission = true,
                isGranted = false,
                hasRequestedBefore = true,
            ),
        )
    }

    @Test
    fun `granted notification permission starts recording`() {
        assertEquals(
            NotificationPermissionDecision.START_RECORDING,
            NotificationPermissionPolicy.decide(
                requiresRuntimePermission = true,
                isGranted = true,
                hasRequestedBefore = false,
            ),
        )
    }
}
