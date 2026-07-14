package com.sona.android.application.recording

import kotlinx.coroutines.flow.StateFlow

interface LiveRecordingController {
    val state: StateFlow<LiveRecordingState>

    suspend fun start()
    suspend fun stop()
}
