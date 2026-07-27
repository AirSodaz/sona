package com.sona.android.adapters.android.settings

import com.sona.android.application.recording.LocalAsrDeviceTier
import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidLocalAsrDeviceCapabilitiesTest {
    @Test
    fun `classifies device tiers from memory storage and cpu`() {
        val gib = 1_024L * 1_024 * 1_024

        assertEquals(LocalAsrDeviceTier.HIGH, classifyLocalAsrDevice(8 * gib, 2 * gib, 8))
        assertEquals(LocalAsrDeviceTier.STANDARD, classifyLocalAsrDevice(4 * gib, gib, 4))
        assertEquals(LocalAsrDeviceTier.LIMITED, classifyLocalAsrDevice(3 * gib, 4 * gib, 8))
    }

    @Test
    fun `recommended threads stay within cpu count`() {
        assertEquals(1, recommendedThreads(LocalAsrDeviceTier.LIMITED, 8))
        assertEquals(2, recommendedThreads(LocalAsrDeviceTier.STANDARD, 8))
        assertEquals(2, recommendedThreads(LocalAsrDeviceTier.HIGH, 2))
    }
}
