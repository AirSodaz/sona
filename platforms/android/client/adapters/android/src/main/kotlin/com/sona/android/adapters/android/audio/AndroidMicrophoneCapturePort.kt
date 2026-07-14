package com.sona.android.adapters.android.audio

import com.sona.android.adapters.android.wav.CheckpointingWavWriter
import com.sona.android.adapters.android.wav.PcmWriter
import com.sona.android.application.recording.MicrophoneCapturePort
import com.sona.android.application.recording.MicrophoneCaptureRequest
import com.sona.android.application.recording.MicrophoneCaptureSession
import com.sona.android.application.recording.RecordingDestination
import kotlinx.coroutines.CoroutineDispatcher

class AndroidMicrophoneCapturePort(
    private val backendFactory: () -> AudioCaptureBackend,
    private val writerFactory: (RecordingDestination) -> PcmWriter = CheckpointingWavWriter::open,
    private val readerDispatcher: CoroutineDispatcher,
) : MicrophoneCapturePort {
    override suspend fun open(request: MicrophoneCaptureRequest): MicrophoneCaptureSession {
        require(request.sampleRateHz == SAMPLE_RATE_HZ) { "Microphone sample rate must be 16000 Hz." }
        require(request.channelCount == CHANNEL_COUNT) { "Microphone capture must be mono." }
        require(request.bitsPerSample == BITS_PER_SAMPLE) { "Microphone capture must use PCM16." }

        val backend = backendFactory()
        val writer = try {
            writerFactory(request.destination)
        } catch (error: Exception) {
            try {
                backend.close()
            } catch (_: Exception) {
                // Resource cleanup must not replace the writer creation failure.
            }
            throw error
        }
        return try {
            AndroidMicrophoneCaptureSession(
                backend = backend,
                writer = writer,
                readerDispatcher = readerDispatcher,
            )
        } catch (error: Exception) {
            try {
                backend.close()
            } catch (_: Exception) {
                // Continue releasing the writer acquired for this session.
            }
            try {
                writer.close()
            } catch (_: Exception) {
                // Construction failure remains the public error.
            }
            throw error
        }
    }

    private companion object {
        const val SAMPLE_RATE_HZ = 16_000
        const val CHANNEL_COUNT = 1
        const val BITS_PER_SAMPLE = 16
    }
}
