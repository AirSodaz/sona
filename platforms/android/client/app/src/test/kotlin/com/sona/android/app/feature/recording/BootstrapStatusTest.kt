package com.sona.android.app.feature.recording

import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.bootstrap.SonaBootstrapSnapshot
import org.junit.Assert.assertFalse
import org.junit.Test

class BootstrapStatusTest {
    @Test
    fun `ready runtime does not render a recording status notice`() {
        assertFalse(shouldShowBootstrapStatus(ready(localRuntimePackaged = true)))
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
