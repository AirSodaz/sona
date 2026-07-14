package com.sona.android.adapters.android.audio

import com.sona.android.adapters.android.wav.PcmWriter
import com.sona.android.application.recording.CapturedAudio
import com.sona.android.application.recording.MicrophoneCaptureRequest
import com.sona.android.application.recording.RecordingDestination
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidMicrophoneCapturePortTest {
    @Test
    fun `open accepts only 16000 Hz mono signed PCM16`() = runBlocking {
        var backendCreations = 0
        var writerCreations = 0
        val port = AndroidMicrophoneCapturePort(
            backendFactory = {
                backendCreations += 1
                IdleBackend
            },
            writerFactory = {
                writerCreations += 1
                IdleWriter
            },
            readerDispatcher = Dispatchers.IO,
        )
        val invalidRequests = listOf(
            request(sampleRateHz = 48_000),
            request(channelCount = 2),
            request(bitsPerSample = 24),
        )

        invalidRequests.forEach { invalid ->
            assertThrows(IllegalArgumentException::class.java) {
                runBlocking { port.open(invalid) }
            }
        }
        val session = port.open(request())

        assertTrue(session is AndroidMicrophoneCaptureSession)
        assertEquals(1, backendCreations)
        assertEquals(1, writerCreations)
        session.close()
    }

    private fun request(
        sampleRateHz: Int = 16_000,
        channelCount: Int = 1,
        bitsPerSample: Int = 16,
    ) = MicrophoneCaptureRequest(
        recordingId = "recording-1",
        destination = RecordingDestination("recording.wav"),
        sampleRateHz = sampleRateHz,
        channelCount = channelCount,
        bitsPerSample = bitsPerSample,
    )
}

private object IdleBackend : AudioCaptureBackend {
    override val bufferSizeBytes = 640

    override fun start() = Unit

    override fun read(buffer: ByteArray): Int = -1

    override fun stop() = Unit

    override fun close() = Unit
}

private object IdleWriter : PcmWriter {
    override fun write(bytes: ByteArray, offset: Int, length: Int) = Unit

    override fun finish() = CapturedAudio(durationMillis = 0, bytesWritten = 0)

    override fun close() = Unit
}
