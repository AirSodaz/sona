package com.sona.android.adapters.android.system

import android.os.SystemClock
import com.sona.android.application.recording.MonotonicClockPort

class AndroidMonotonicClock(
    private val elapsedRealtimeSupplier: () -> Long = SystemClock::elapsedRealtime,
) : MonotonicClockPort {
    override fun elapsedRealtimeMillis(): Long = elapsedRealtimeSupplier()
}
