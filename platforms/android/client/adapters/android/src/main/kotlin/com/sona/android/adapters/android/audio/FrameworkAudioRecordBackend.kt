package com.sona.android.adapters.android.audio

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioRecordingConfiguration
import android.media.MediaRecorder
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.annotation.RequiresPermission
import com.sona.android.application.recording.AudioInputEvent
import java.io.IOException

private const val PCM_FRAME_SIZE_BYTES = 640

internal data class FrameworkAudioRecordConstructionPolicy(
    val audioSource: Int,
    val sampleRateHz: Int,
    val channelMask: Int,
    val encoding: Int,
    val frameSizeBytes: Int,
    val bufferSizeBytes: Int,
    val privacySensitive: Boolean,
) {
    companion object {
        fun create(
            minBufferSizeBytes: Int,
            apiLevel: Int,
        ): FrameworkAudioRecordConstructionPolicy {
            if (minBufferSizeBytes <= 0) {
                throw IllegalStateException("AudioRecord minimum buffer size is unavailable.")
            }
            val doubledMinimum = minBufferSizeBytes.toLong() * 2L
            val alignedBuffer =
                ((doubledMinimum + PCM_FRAME_SIZE_BYTES - 1L) / PCM_FRAME_SIZE_BYTES) *
                    PCM_FRAME_SIZE_BYTES
            if (alignedBuffer > Int.MAX_VALUE) {
                throw IllegalStateException("AudioRecord minimum buffer size is too large.")
            }
            return FrameworkAudioRecordConstructionPolicy(
                audioSource = MediaRecorder.AudioSource.VOICE_RECOGNITION,
                sampleRateHz = SAMPLE_RATE_HZ,
                channelMask = AudioFormat.CHANNEL_IN_MONO,
                encoding = AudioFormat.ENCODING_PCM_16BIT,
                frameSizeBytes = PCM_FRAME_SIZE_BYTES,
                bufferSizeBytes = alignedBuffer.toInt(),
                privacySensitive = apiLevel >= API_WITH_PRIVACY_SENSITIVE,
            )
        }

        private const val SAMPLE_RATE_HZ = 16_000
        private const val API_WITH_PRIVACY_SENSITIVE = 30
    }
}

internal class AudioCaptureReadException : IOException("Audio capture read failed.")

internal interface PlatformAudioRecord {
    val audioSessionId: Int

    fun start()
    fun read(buffer: ByteArray): Int
    fun stop()
    fun release()
}

internal fun interface PlatformRecordingCallback {
    fun onRecordingConfigurationsChanged(configurations: List<AudioRecordingSnapshot>)
}

internal interface PlatformAudioRecordingMonitor {
    fun createCallback(
        listener: (List<AudioRecordingSnapshot>) -> Unit,
    ): PlatformRecordingCallback

    fun register(callback: PlatformRecordingCallback)
    fun unregister(callback: PlatformRecordingCallback)
}

class FrameworkAudioRecordBackend internal constructor(
    private val record: PlatformAudioRecord,
    private val apiLevel: Int,
    private val monitor: PlatformAudioRecordingMonitor?,
    override val bufferSizeBytes: Int = PCM_FRAME_SIZE_BYTES,
) : AudioCaptureBackend, AudioInputMonitoringBackend {
    private val lifecycleLock = Any()
    private val mapper = AudioInputEventMapper(apiLevel, record.audioSessionId)

    private var listener: AudioInputEventListener? = null
    private var generation = 0L
    private var started = false
    private var stopping = false
    private var closed = false
    private var registeredCallback: PlatformRecordingCallback? = null

    init {
        require(bufferSizeBytes > 0) { "AudioRecord buffer size must be positive." }
        require((apiLevel >= API_WITH_MONITORING) == (monitor != null)) {
            "Audio recording monitoring must match the Android API level."
        }
    }

    override fun setInputEventListener(listener: AudioInputEventListener?) {
        synchronized(lifecycleLock) {
            this.listener = listener
        }
    }

    override fun start() {
        var currentGeneration = 0L
        var startEvents = emptyList<AudioInputEvent>()
        synchronized(lifecycleLock) {
            check(!closed) { "AudioRecord backend is closed." }
            check(!started && !stopping) { "AudioRecord backend has already started." }
            currentGeneration = ++generation
            val callback = monitor?.createCallback { configurations ->
                deliverConfigurations(currentGeneration, configurations)
            }
            registeredCallback = callback

            try {
                if (callback != null) {
                    monitor!!.register(callback)
                }
                record.start()
                started = true
                if (callback == null) {
                    startEvents = mapper.onMonitoringStarted()
                }
            } catch (error: Throwable) {
                generation += 1
                stopping = true
                registeredCallback = null
                if (callback != null) {
                    try {
                        monitor?.unregister(callback)
                    } catch (_: Throwable) {
                        // The start failure is the stable public failure for this path.
                    }
                }
                throw error
            }
        }
        deliverEvents(currentGeneration, startEvents)
    }

    override fun read(buffer: ByteArray): Int {
        if (isExpectedStop()) {
            return END_OF_STREAM
        }
        val bytesRead = try {
            record.read(buffer)
        } catch (_: Exception) {
            if (isExpectedStop()) {
                return END_OF_STREAM
            }
            throw AudioCaptureReadException()
        }
        if (bytesRead < 0) {
            if (isExpectedStop()) {
                return END_OF_STREAM
            }
            throw AudioCaptureReadException()
        }
        return bytesRead
    }

    override fun stop() = synchronized(lifecycleLock) {
        stopLocked()
    }

    private fun stopLocked() {
        val callback: PlatformRecordingCallback?
        val shouldStop: Boolean
        if (closed || stopping) {
            return
        }
        stopping = true
        generation += 1
        shouldStop = started
        started = false
        callback = registeredCallback
        registeredCallback = null

        var failure: Throwable? = null
        if (shouldStop) {
            try {
                record.stop()
            } catch (error: Throwable) {
                failure = error
            }
        }
        if (callback != null) {
            try {
                monitor?.unregister(callback)
            } catch (error: Throwable) {
                if (failure == null) {
                    failure = error
                }
            }
        }
        failure?.let { throw it }
    }

    override fun close() {
        synchronized(lifecycleLock) {
            var failure: Throwable? = null
            try {
                stopLocked()
            } catch (error: Throwable) {
                failure = error
            }

            val shouldRelease = if (closed) {
                false
            } else {
                closed = true
                generation += 1
                listener = null
                true
            }
            if (shouldRelease) {
                try {
                    record.release()
                } catch (error: Throwable) {
                    if (failure == null) {
                        failure = error
                    }
                }
            }
            failure?.let { throw it }
        }
    }

    private fun deliverConfigurations(
        callbackGeneration: Long,
        configurations: List<AudioRecordingSnapshot>,
    ) {
        val events = synchronized(lifecycleLock) {
            if (!acceptsCallback(callbackGeneration)) {
                return
            }
            mapper.onRecordingConfigurationsChanged(configurations)
        }
        deliverEvents(callbackGeneration, events)
    }

    private fun deliverEvents(
        callbackGeneration: Long,
        events: List<AudioInputEvent>,
    ) {
        events.forEach { event ->
            synchronized(lifecycleLock) {
                if (acceptsCallback(callbackGeneration)) {
                    listener?.onInputEvent(event)
                }
            }
        }
    }

    private fun acceptsCallback(callbackGeneration: Long): Boolean =
        started && !closed && !stopping && generation == callbackGeneration

    private fun isExpectedStop(): Boolean = synchronized(lifecycleLock) {
        stopping || closed
    }

    companion object {
        private const val API_WITH_MONITORING = 24
        private const val END_OF_STREAM = -1

        @JvmStatic
        @RequiresPermission(Manifest.permission.RECORD_AUDIO)
        fun create(context: Context): FrameworkAudioRecordBackend {
            val apiLevel = Build.VERSION.SDK_INT
            val minBufferSize = AudioRecord.getMinBufferSize(
                16_000,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
            )
            val policy = FrameworkAudioRecordConstructionPolicy.create(minBufferSize, apiLevel)
            val builder = AudioRecord.Builder()
                .setAudioSource(policy.audioSource)
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(policy.sampleRateHz)
                        .setChannelMask(policy.channelMask)
                        .setEncoding(policy.encoding)
                        .build(),
                )
                .setBufferSizeInBytes(policy.bufferSizeBytes)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                Api30AudioRecordBuilder.configurePrivacySensitive(builder)
            }
            val audioRecord = builder.build()
            if (audioRecord.state != AudioRecord.STATE_INITIALIZED) {
                audioRecord.release()
                throw IllegalStateException("AudioRecord failed to initialize.")
            }
            val monitor = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager
                if (audioManager == null) {
                    audioRecord.release()
                    throw IllegalStateException("Audio recording monitoring is unavailable.")
                }
                Api24PlatformAudioRecordingMonitor(audioManager)
            } else {
                null
            }
            return FrameworkAudioRecordBackend(
                record = AndroidPlatformAudioRecord(audioRecord),
                apiLevel = apiLevel,
                monitor = monitor,
                bufferSizeBytes = policy.bufferSizeBytes,
            )
        }
    }
}

private class AndroidPlatformAudioRecord(
    private val audioRecord: AudioRecord,
) : PlatformAudioRecord {
    override val audioSessionId: Int
        get() = audioRecord.audioSessionId

    @SuppressLint("MissingPermission")
    override fun start() = audioRecord.startRecording()

    override fun read(buffer: ByteArray): Int =
        audioRecord.read(buffer, 0, buffer.size, AudioRecord.READ_BLOCKING)

    override fun stop() = audioRecord.stop()

    override fun release() = audioRecord.release()
}

@RequiresApi(Build.VERSION_CODES.N)
private class Api24PlatformRecordingCallback(
    private val listener: (List<AudioRecordingSnapshot>) -> Unit,
) : AudioManager.AudioRecordingCallback(), PlatformRecordingCallback {
    override fun onRecordingConfigChanged(configs: MutableList<AudioRecordingConfiguration>) {
        onRecordingConfigurationsChanged(configs.map { it.toSnapshot() })
    }

    override fun onRecordingConfigurationsChanged(configurations: List<AudioRecordingSnapshot>) {
        listener(configurations)
    }
}

@RequiresApi(Build.VERSION_CODES.N)
private class Api24PlatformAudioRecordingMonitor(
    private val audioManager: AudioManager,
) : PlatformAudioRecordingMonitor {
    override fun createCallback(
        listener: (List<AudioRecordingSnapshot>) -> Unit,
    ): PlatformRecordingCallback = Api24PlatformRecordingCallback(listener)

    override fun register(callback: PlatformRecordingCallback) {
        audioManager.registerAudioRecordingCallback(
            callback as Api24PlatformRecordingCallback,
            null,
        )
    }

    override fun unregister(callback: PlatformRecordingCallback) {
        audioManager.unregisterAudioRecordingCallback(callback as Api24PlatformRecordingCallback)
    }
}

@RequiresApi(Build.VERSION_CODES.N)
private fun AudioRecordingConfiguration.toSnapshot(): AudioRecordingSnapshot {
    val clientFormat = clientFormat
    return AudioRecordingSnapshot(
        clientAudioSessionId = clientAudioSessionId,
        silenced = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            Api29AudioRecordingConfiguration.isClientSilenced(this)
        } else {
            null
        },
        deviceName = audioDevice?.productName?.toString(),
        sampleRateHz = clientFormat.sampleRate.takeIf { it > 0 },
        channelCount = clientFormat.channelCount.takeIf { it > 0 },
        preprocessing = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            Api29AudioRecordingConfiguration.preprocessingNames(this)
        } else {
            emptyList()
        },
    )
}

@RequiresApi(Build.VERSION_CODES.Q)
private object Api29AudioRecordingConfiguration {
    fun isClientSilenced(configuration: AudioRecordingConfiguration): Boolean =
        configuration.isClientSilenced

    fun preprocessingNames(configuration: AudioRecordingConfiguration): List<String> =
        configuration.effects.map { it.name }.sorted()
}

@RequiresApi(Build.VERSION_CODES.R)
private object Api30AudioRecordBuilder {
    fun configurePrivacySensitive(builder: AudioRecord.Builder) {
        builder.setPrivacySensitive(true)
    }
}
