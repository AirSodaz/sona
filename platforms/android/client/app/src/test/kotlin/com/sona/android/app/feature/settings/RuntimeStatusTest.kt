package com.sona.android.app.feature.settings

import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.bootstrap.SonaBootstrapSnapshot
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RuntimeStatusTest {
    @Test
    fun `packaged runtime does not render a settings status card`() {
        assertFalse(shouldShowRuntimeStatus(ready(localRuntimePackaged = true)))
    }

    @Test
    fun `unavailable loading and error runtime states remain visible`() {
        assertTrue(shouldShowRuntimeStatus(ready(localRuntimePackaged = false)))
        assertTrue(shouldShowRuntimeStatus(SonaBootstrapUiState.Loading))
        assertTrue(shouldShowRuntimeStatus(SonaBootstrapUiState.Error("failed")))
    }

    private fun ready(localRuntimePackaged: Boolean) = SonaBootstrapUiState.Ready(
        SonaBootstrapSnapshot(
            defaultConfigJson = "{}",
            onlineStreamingAvailable = true,
            localRuntimePackaged = localRuntimePackaged,
            localStreamingSessionAvailable = true,
        ),
    )
}
