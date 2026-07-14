package com.sona.android.adapters.android.system

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidMonotonicClockTest {
    @Test
    fun `elapsed realtime supplier remains injectable`() {
        val clock = AndroidMonotonicClock(elapsedRealtimeSupplier = { 12_345L })

        assertEquals(12_345L, clock.elapsedRealtimeMillis())
    }
}
