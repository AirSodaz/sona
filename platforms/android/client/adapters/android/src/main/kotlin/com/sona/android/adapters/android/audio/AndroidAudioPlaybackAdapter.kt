package com.sona.android.adapters.android.audio

import android.content.Context
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import com.sona.android.application.media.AudioPlaybackFailure
import com.sona.android.application.media.AudioPlaybackPort
import com.sona.android.application.media.AudioPlaybackState
import com.sona.android.application.media.AudioPlaybackStatus
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class AndroidAudioPlaybackAdapter private constructor(context: Context) : AudioPlaybackPort {
    private val appContext = context.applicationContext
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val mutableState = MutableStateFlow(AudioPlaybackState())
    override val state: StateFlow<AudioPlaybackState> = mutableState.asStateFlow()
    private var player: ExoPlayer? = null
    private var positionJob: Job? = null

    override suspend fun prepare(historyId: String, nativePath: String) {
        release()
        val source = runCatching { File(nativePath).canonicalFile }.getOrNull()
        if (historyId.isBlank() || source?.isFile != true) {
            mutableState.value = AudioPlaybackState(
                historyId = historyId.takeIf(String::isNotBlank),
                status = AudioPlaybackStatus.Failed(AudioPlaybackFailure.UNAVAILABLE),
            )
            return
        }
        mutableState.value = AudioPlaybackState(historyId, AudioPlaybackStatus.Preparing)
        val created = runCatching { ExoPlayer.Builder(appContext).build() }.getOrElse {
            mutableState.value = AudioPlaybackState(
                historyId = historyId,
                status = AudioPlaybackStatus.Failed(AudioPlaybackFailure.PLAYBACK_FAILED),
            )
            return
        }
        player = created
        runCatching {
            created.setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                    .build(),
                true,
            )
            created.setHandleAudioBecomingNoisy(true)
            created.addListener(listener)
            created.setMediaItem(MediaItem.fromUri(source.toURI().toString()))
            created.prepare()
        }.onFailure {
            created.removeListener(listener)
            created.release()
            player = null
            mutableState.value = AudioPlaybackState(
                historyId = historyId,
                status = AudioPlaybackStatus.Failed(AudioPlaybackFailure.PLAYBACK_FAILED),
            )
            return
        }
    }

    override fun play() {
        val current = player ?: return
        if (current.playbackState == Player.STATE_ENDED) current.seekTo(0)
        current.play()
    }

    override fun pause() {
        player?.pause()
    }

    override fun seekTo(positionMillis: Long) {
        val current = player ?: return
        val duration = current.duration.takeIf { it != C.TIME_UNSET && it >= 0 } ?: Long.MAX_VALUE
        current.seekTo(positionMillis.coerceIn(0L, duration))
        publishPosition(current)
    }

    override fun seekBy(deltaMillis: Long) {
        val current = player ?: return
        seekTo(current.currentPosition + deltaMillis)
    }

    override fun setSpeed(speed: Float) {
        if (speed !in SUPPORTED_SPEEDS) return
        player?.setPlaybackSpeed(speed)
        mutableState.value = mutableState.value.copy(speed = speed)
    }

    override fun release() {
        positionJob?.cancel()
        positionJob = null
        player?.removeListener(listener)
        player?.release()
        player = null
        mutableState.value = AudioPlaybackState()
    }

    private val listener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            val current = player ?: return
            val status = when (playbackState) {
                Player.STATE_BUFFERING -> AudioPlaybackStatus.Preparing
                Player.STATE_READY -> if (current.isPlaying) {
                    AudioPlaybackStatus.Playing
                } else if (current.currentPosition > 0) {
                    AudioPlaybackStatus.Paused
                } else {
                    AudioPlaybackStatus.Ready
                }
                Player.STATE_ENDED -> AudioPlaybackStatus.Ended
                else -> return
            }
            publish(current, status)
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            val current = player ?: return
            val status = if (isPlaying) AudioPlaybackStatus.Playing else when (current.playbackState) {
                Player.STATE_ENDED -> AudioPlaybackStatus.Ended
                Player.STATE_READY -> if (current.currentPosition > 0) {
                    AudioPlaybackStatus.Paused
                } else {
                    AudioPlaybackStatus.Ready
                }
                else -> AudioPlaybackStatus.Preparing
            }
            publish(current, status)
            if (isPlaying) startPositionUpdates() else positionJob?.cancel()
        }

        override fun onPlayerError(error: PlaybackException) {
            positionJob?.cancel()
            val failure = when (error.errorCode) {
                PlaybackException.ERROR_CODE_DECODING_FAILED,
                PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
                -> AudioPlaybackFailure.DECODE_FAILED
                PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED,
                -> AudioPlaybackFailure.UNSUPPORTED
                PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND -> AudioPlaybackFailure.UNAVAILABLE
                else -> AudioPlaybackFailure.PLAYBACK_FAILED
            }
            mutableState.value = mutableState.value.copy(status = AudioPlaybackStatus.Failed(failure))
        }
    }

    private fun startPositionUpdates() {
        positionJob?.cancel()
        positionJob = scope.launch {
            while (isActive) {
                player?.let(::publishPosition)
                delay(POSITION_UPDATE_MILLIS)
            }
        }
    }

    private fun publishPosition(current: Player) = publish(current, mutableState.value.status)

    private fun publish(current: Player, status: AudioPlaybackStatus) {
        val duration = current.duration.takeIf { it != C.TIME_UNSET && it >= 0 } ?: 0L
        mutableState.value = mutableState.value.copy(
            status = status,
            positionMillis = current.currentPosition.coerceAtLeast(0L),
            durationMillis = duration,
            bufferedPositionMillis = current.bufferedPosition.coerceAtLeast(0L),
            speed = current.playbackParameters.speed,
        )
    }

    companion object {
        val SUPPORTED_SPEEDS = setOf(0.5f, 0.8f, 1f, 1.25f, 1.5f, 2f, 3f)
        private const val POSITION_UPDATE_MILLIS = 250L

        fun create(context: Context): AndroidAudioPlaybackAdapter = AndroidAudioPlaybackAdapter(context)
    }
}
