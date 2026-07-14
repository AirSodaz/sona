package com.sona.android.adapters.android.audio

import android.media.AudioFormat
import android.media.MediaRecorder
import com.sona.android.application.recording.AudioInputEvent
import java.io.IOException
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.Future
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class FrameworkAudioRecordBackendTest {
    @Test
    fun `construction policy fixes source format and aligned double minimum buffer`() {
        val policy = FrameworkAudioRecordConstructionPolicy.create(
            minBufferSizeBytes = 641,
            apiLevel = 29,
        )

        assertEquals(MediaRecorder.AudioSource.VOICE_RECOGNITION, policy.audioSource)
        assertEquals(16_000, policy.sampleRateHz)
        assertEquals(AudioFormat.CHANNEL_IN_MONO, policy.channelMask)
        assertEquals(AudioFormat.ENCODING_PCM_16BIT, policy.encoding)
        assertEquals(640, policy.frameSizeBytes)
        assertEquals(1_920, policy.bufferSizeBytes)
        assertTrue(policy.bufferSizeBytes >= 2 * 641)
        assertEquals(0, policy.bufferSizeBytes % policy.frameSizeBytes)
        assertFalse(policy.privacySensitive)
    }

    @Test
    fun `construction policy rejects unavailable minimum buffer results`() {
        listOf(-2, -1, 0).forEach { invalid ->
            val thrown = assertThrows(IllegalStateException::class.java) {
                FrameworkAudioRecordConstructionPolicy.create(
                    minBufferSizeBytes = invalid,
                    apiLevel = 30,
                )
            }
            assertEquals("AudioRecord minimum buffer size is unavailable.", thrown.message)
        }
    }

    @Test
    fun `construction policy enables privacy only on API 30 and newer`() {
        assertFalse(policy(apiLevel = 29).privacySensitive)
        assertTrue(policy(apiLevel = 30).privacySensitive)
        assertTrue(policy(apiLevel = 37).privacySensitive)
    }

    @Test
    fun `API 23 emits unavailable without creating a callback`() {
        val record = FakePlatformAudioRecord()
        val events = mutableListOf<AudioInputEvent>()
        val backend = FrameworkAudioRecordBackend(
            record = record,
            apiLevel = 23,
            monitor = null,
        )
        backend.setInputEventListener(events::add)

        backend.start()

        assertEquals(listOf(AudioInputEvent.MonitoringUnavailable), events)
        assertEquals(listOf("start"), record.calls)
        backend.stop()
        backend.close()
    }

    @Test
    fun `callback is registered before start and the exact instance is unregistered once`() {
        val calls = Collections.synchronizedList(mutableListOf<String>())
        val record = FakePlatformAudioRecord(calls = calls)
        val monitor = FakePlatformAudioRecordingMonitor(calls)
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 29, monitor = monitor)

        backend.start()
        backend.stop()
        backend.stop()
        backend.close()
        backend.close()

        assertEquals(listOf("register", "start", "stop", "unregister", "release"), calls)
        assertEquals(1, monitor.registeredCallbacks.size)
        assertEquals(1, monitor.unregisteredCallbacks.size)
        assertSame(monitor.registeredCallbacks.single(), monitor.unregisteredCallbacks.single())
        assertEquals(1, record.releaseCalls)
    }

    @Test
    fun `start failure unregisters once and close releases once`() {
        val failure = IOException("platform start failed")
        val calls = Collections.synchronizedList(mutableListOf<String>())
        val record = FakePlatformAudioRecord(startFailure = failure, calls = calls)
        val monitor = FakePlatformAudioRecordingMonitor(calls)
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 29, monitor = monitor)

        assertSame(failure, assertThrows(IOException::class.java) { backend.start() })
        backend.stop()
        backend.close()
        backend.close()

        assertEquals(1, monitor.registeredCallbacks.size)
        assertEquals(1, monitor.unregisteredCallbacks.size)
        assertSame(monitor.registeredCallbacks.single(), monitor.unregisteredCallbacks.single())
        assertEquals(1, record.releaseCalls)
        assertEquals(listOf("register", "start", "unregister", "release"), calls)
    }

    @Test
    fun `stop waits for in-flight start before stopping and cleaning the exact callback`() {
        val registerEntered = CountDownLatch(1)
        val allowRegister = CountDownLatch(1)
        val calls = Collections.synchronizedList(mutableListOf<String>())
        val record = FakePlatformAudioRecord(calls = calls)
        val monitor = FakePlatformAudioRecordingMonitor(
            calls = calls,
            registerEntered = registerEntered,
            allowRegister = allowRegister,
        )
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 29, monitor = monitor)
        val executor = Executors.newFixedThreadPool(2)
        val startFuture = executor.submit { backend.start() }
        var stopFuture: Future<*>? = null

        try {
            assertTrue(registerEntered.await(5, TimeUnit.SECONDS))
            val stopAttempted = CountDownLatch(1)
            stopFuture = executor.submit {
                stopAttempted.countDown()
                backend.stop()
            }
            assertTrue(stopAttempted.await(5, TimeUnit.SECONDS))

            assertThrows(TimeoutException::class.java) {
                stopFuture.get(200, TimeUnit.MILLISECONDS)
            }
            assertEquals(0, record.startCalls)

            allowRegister.countDown()
            startFuture.get(5, TimeUnit.SECONDS)
            stopFuture.get(5, TimeUnit.SECONDS)
            backend.close()

            assertEquals(listOf("register", "start", "stop", "unregister", "release"), calls)
            assertEquals(1, record.startCalls)
            assertEquals(1, record.stopCalls)
            assertEquals(1, record.releaseCalls)
            assertEquals(1, monitor.registeredCallbacks.size)
            assertEquals(1, monitor.unregisteredCallbacks.size)
            assertSame(monitor.registeredCallbacks.single(), monitor.unregisteredCallbacks.single())
        } finally {
            allowRegister.countDown()
            runCatching { startFuture.get(5, TimeUnit.SECONDS) }
            stopFuture?.let { runCatching { it.get(5, TimeUnit.SECONDS) } }
            runCatching { backend.close() }
            executor.shutdownNow()
        }
    }

    @Test
    fun `close waits for in-flight stop before unregister and release`() {
        val stopEntered = CountDownLatch(1)
        val allowStop = CountDownLatch(1)
        val calls = Collections.synchronizedList(mutableListOf<String>())
        val record = FakePlatformAudioRecord(
            calls = calls,
            stopEntered = stopEntered,
            allowStop = allowStop,
        )
        val monitor = FakePlatformAudioRecordingMonitor(calls)
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 29, monitor = monitor)
        val executor = Executors.newFixedThreadPool(2)
        backend.start()
        val stopFuture = executor.submit { backend.stop() }
        var closeFuture: Future<*>? = null

        try {
            assertTrue(stopEntered.await(5, TimeUnit.SECONDS))
            val closeAttempted = CountDownLatch(1)
            closeFuture = executor.submit {
                closeAttempted.countDown()
                backend.close()
            }
            assertTrue(closeAttempted.await(5, TimeUnit.SECONDS))

            assertThrows(TimeoutException::class.java) {
                closeFuture.get(200, TimeUnit.MILLISECONDS)
            }
            assertEquals(0, record.releaseCalls)
            assertEquals(0, monitor.unregisteredCallbacks.size)

            allowStop.countDown()
            stopFuture.get(5, TimeUnit.SECONDS)
            closeFuture.get(5, TimeUnit.SECONDS)

            assertEquals(listOf("register", "start", "stop", "unregister", "release"), calls)
            assertEquals(1, record.stopCalls)
            assertEquals(1, record.releaseCalls)
            assertEquals(1, monitor.unregisteredCallbacks.size)
            assertSame(monitor.registeredCallbacks.single(), monitor.unregisteredCallbacks.single())
        } finally {
            allowStop.countDown()
            runCatching { stopFuture.get(5, TimeUnit.SECONDS) }
            closeFuture?.let { runCatching { it.get(5, TimeUnit.SECONDS) } }
            runCatching { backend.close() }
            executor.shutdownNow()
        }
    }

    @Test
    fun `matching callback input is delivered and late callbacks are ignored`() {
        val record = FakePlatformAudioRecord(audioSessionId = 41)
        val monitor = FakePlatformAudioRecordingMonitor()
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 29, monitor = monitor)
        val events = mutableListOf<AudioInputEvent>()
        backend.setInputEventListener(events::add)
        backend.start()
        val callback = monitor.registeredCallbacks.single()

        callback.onRecordingConfigurationsChanged(
            listOf(
                snapshot(sessionId = 7, deviceName = "wrong"),
                snapshot(sessionId = 41, deviceName = "target"),
            ),
        )
        backend.stop()
        callback.onRecordingConfigurationsChanged(
            listOf(snapshot(sessionId = 41, silenced = true)),
        )
        backend.close()
        callback.onRecordingConfigurationsChanged(
            listOf(snapshot(sessionId = 41, silenced = true)),
        )

        assertEquals(2, events.size)
        assertEquals(AudioInputEvent.Active, events[0])
        assertTrue(events[1] is AudioInputEvent.ConfigurationChanged)
    }

    @Test
    fun `non-stop negative and exceptional reads use stable adapter failure`() {
        val record = FakePlatformAudioRecord(readResult = -3)
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 23, monitor = null)
        backend.start()

        val negative = assertThrows(AudioCaptureReadException::class.java) {
            backend.read(ByteArray(640))
        }
        assertEquals("Audio capture read failed.", negative.message)

        record.readFailure = IOException("driver detail")
        val exceptional = assertThrows(AudioCaptureReadException::class.java) {
            backend.read(ByteArray(640))
        }
        assertEquals("Audio capture read failed.", exceptional.message)
        backend.stop()
        backend.close()
    }

    @Test
    fun `read errors caused by expected stop become end of stream`() {
        val record = FakePlatformAudioRecord(readResult = -3)
        val backend = FrameworkAudioRecordBackend(record, apiLevel = 23, monitor = null)
        backend.start()
        backend.stop()

        assertEquals(-1, backend.read(ByteArray(640)))
        record.readFailure = IOException("stopped")
        assertEquals(-1, backend.read(ByteArray(640)))
        backend.close()
    }

    private fun policy(apiLevel: Int) = FrameworkAudioRecordConstructionPolicy.create(
        minBufferSizeBytes = 640,
        apiLevel = apiLevel,
    )

    private fun snapshot(
        sessionId: Int,
        silenced: Boolean? = false,
        deviceName: String? = "microphone",
    ) = AudioRecordingSnapshot(
        clientAudioSessionId = sessionId,
        silenced = silenced,
        deviceName = deviceName,
        sampleRateHz = 16_000,
        channelCount = 1,
        preprocessing = emptyList(),
    )
}

private class FakePlatformAudioRecord(
    override val audioSessionId: Int = 9,
    private val startFailure: IOException? = null,
    var readResult: Int = 640,
    val calls: MutableList<String> = Collections.synchronizedList(mutableListOf()),
    private val stopEntered: CountDownLatch? = null,
    private val allowStop: CountDownLatch? = null,
) : PlatformAudioRecord {
    var readFailure: IOException? = null
    var startCalls = 0
    var stopCalls = 0
    var releaseCalls = 0

    override fun start() {
        calls += "start"
        startCalls += 1
        startFailure?.let { throw it }
    }

    override fun read(buffer: ByteArray): Int {
        readFailure?.let { throw it }
        return readResult
    }

    override fun stop() {
        calls += "stop"
        stopCalls += 1
        stopEntered?.countDown()
        if (allowStop != null && !allowStop.await(5, TimeUnit.SECONDS)) {
            throw AssertionError("Timed out waiting to release platform stop.")
        }
    }

    override fun release() {
        calls += "release"
        releaseCalls += 1
    }
}

private class FakePlatformRecordingCallback(
    private val listener: (List<AudioRecordingSnapshot>) -> Unit,
) : PlatformRecordingCallback {
    override fun onRecordingConfigurationsChanged(configurations: List<AudioRecordingSnapshot>) {
        listener(configurations)
    }
}

private class FakePlatformAudioRecordingMonitor(
    private val calls: MutableList<String> = Collections.synchronizedList(mutableListOf()),
    private val registerEntered: CountDownLatch? = null,
    private val allowRegister: CountDownLatch? = null,
) : PlatformAudioRecordingMonitor {
    val registeredCallbacks = Collections.synchronizedList(mutableListOf<PlatformRecordingCallback>())
    val unregisteredCallbacks = Collections.synchronizedList(mutableListOf<PlatformRecordingCallback>())

    override fun createCallback(
        listener: (List<AudioRecordingSnapshot>) -> Unit,
    ): PlatformRecordingCallback = FakePlatformRecordingCallback(listener)

    override fun register(callback: PlatformRecordingCallback) {
        registerEntered?.countDown()
        if (allowRegister != null && !allowRegister.await(5, TimeUnit.SECONDS)) {
            throw AssertionError("Timed out waiting to release callback registration.")
        }
        calls += "register"
        registeredCallbacks += callback
    }

    override fun unregister(callback: PlatformRecordingCallback) {
        calls += "unregister"
        unregisteredCallbacks += callback
    }
}
