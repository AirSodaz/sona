package com.sona.android.adapters.android.audio

import android.media.AudioFormat
import android.media.MediaRecorder
import android.os.Build
import com.sona.android.application.recording.AudioInputEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import androidx.test.ext.junit.runners.AndroidJUnit4

@RunWith(AndroidJUnit4::class)
class FrameworkAudioRecordBackendInstrumentedTest {
    @Test
    fun constructionPolicyIsStableAcrossSupportedApiLevels() {
        val policy = FrameworkAudioRecordConstructionPolicy.create(
            minBufferSizeBytes = 641,
            apiLevel = Build.VERSION.SDK_INT,
        )

        assertEquals(MediaRecorder.AudioSource.VOICE_RECOGNITION, policy.audioSource)
        assertEquals(16_000, policy.sampleRateHz)
        assertEquals(AudioFormat.CHANNEL_IN_MONO, policy.channelMask)
        assertEquals(AudioFormat.ENCODING_PCM_16BIT, policy.encoding)
        assertEquals(1_920, policy.bufferSizeBytes)
        assertEquals(0, policy.bufferSizeBytes % 640)
        assertEquals(Build.VERSION.SDK_INT >= 30, policy.privacySensitive)
    }

    @Test
    fun api23LoadsAndStartsWithoutLinkingMonitoringOrPrivacyApis() {
        if (Build.VERSION.SDK_INT != 23) {
            return
        }
        assertNotNull(FrameworkAudioRecordBackend::class.java)
        val record = SmokePlatformAudioRecord()
        val events = mutableListOf<AudioInputEvent>()
        val backend = FrameworkAudioRecordBackend(
            record = record,
            apiLevel = Build.VERSION.SDK_INT,
            monitor = null,
        )
        backend.setInputEventListener(events::add)

        backend.start()
        backend.stop()
        backend.close()

        assertEquals(listOf(AudioInputEvent.MonitoringUnavailable), events)
        assertTrue(record.started)
        assertTrue(record.stopped)
        assertTrue(record.released)
        assertFalse(
            FrameworkAudioRecordConstructionPolicy.create(320, Build.VERSION.SDK_INT)
                .privacySensitive,
        )
    }
}

private class SmokePlatformAudioRecord : PlatformAudioRecord {
    override val audioSessionId = 17
    var started = false
    var stopped = false
    var released = false

    override fun start() {
        started = true
    }

    override fun read(buffer: ByteArray): Int = -1

    override fun stop() {
        stopped = true
    }

    override fun release() {
        released = true
    }
}
