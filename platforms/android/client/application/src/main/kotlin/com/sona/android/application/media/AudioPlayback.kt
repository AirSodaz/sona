package com.sona.android.application.media

import kotlinx.coroutines.flow.StateFlow

enum class AudioPlaybackFailure { UNAVAILABLE, DECODE_FAILED, UNSUPPORTED, PLAYBACK_FAILED }

sealed interface AudioPlaybackStatus {
    data object Idle : AudioPlaybackStatus
    data object Preparing : AudioPlaybackStatus
    data object Ready : AudioPlaybackStatus
    data object Playing : AudioPlaybackStatus
    data object Paused : AudioPlaybackStatus
    data object Ended : AudioPlaybackStatus
    data class Failed(val reason: AudioPlaybackFailure) : AudioPlaybackStatus
}

data class AudioPlaybackState(
    val historyId: String? = null,
    val status: AudioPlaybackStatus = AudioPlaybackStatus.Idle,
    val positionMillis: Long = 0,
    val durationMillis: Long = 0,
    val bufferedPositionMillis: Long = 0,
    val speed: Float = 1f,
)

interface AudioPlaybackPort {
    val state: StateFlow<AudioPlaybackState>
    suspend fun prepare(historyId: String, nativePath: String)
    fun play()
    fun pause()
    fun seekTo(positionMillis: Long)
    fun seekBy(deltaMillis: Long)
    fun setSpeed(speed: Float)
    fun release()
}
