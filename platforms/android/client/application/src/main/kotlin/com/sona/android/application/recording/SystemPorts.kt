package com.sona.android.application.recording

fun interface WallClockPort {
    fun nowEpochMillis(): Long
}

fun interface MonotonicClockPort {
    fun elapsedRealtimeMillis(): Long
}

fun interface RecordingIdPort {
    fun nextRecordingId(): String
}
