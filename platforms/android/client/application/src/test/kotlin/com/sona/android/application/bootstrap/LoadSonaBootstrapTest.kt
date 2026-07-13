package com.sona.android.application.bootstrap

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class LoadSonaBootstrapTest {
    @Test
    fun `loads one validated snapshot through the application port`() {
        val expected = SonaBootstrapSnapshot(
            defaultConfigJson = "{\"version\":1}",
            onlineStreamingAvailable = true,
            localRuntimePackaged = true,
            localStreamingSessionAvailable = false,
        )
        var calls = 0
        val useCase = LoadSonaBootstrap(
            port = SonaBootstrapPort {
                calls += 1
                expected
            },
        )

        assertEquals(expected, useCase())
        assertEquals(1, calls)
    }

    @Test
    fun `rejects a snapshot without canonical default config JSON`() {
        val useCase = LoadSonaBootstrap(
            port = SonaBootstrapPort {
                SonaBootstrapSnapshot(
                    defaultConfigJson = "   ",
                    onlineStreamingAvailable = true,
                    localRuntimePackaged = true,
                    localStreamingSessionAvailable = false,
                )
            },
        )

        assertThrows(IllegalArgumentException::class.java) {
            useCase()
        }
    }
}
