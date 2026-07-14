package com.sona.android.adapters.android.audio

import com.sona.android.adapters.android.wav.PcmWriter
import com.sona.android.application.recording.AudioInputEvent
import com.sona.android.application.recording.CapturedAudio
import com.sona.android.application.recording.MicrophoneCaptureEvent
import com.sona.android.application.recording.MicrophoneCaptureFailureKind
import java.io.IOException
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.atomic.AtomicReference
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AndroidMicrophoneCaptureSessionTest {
    private val readerDispatcher =
        Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "test-capture-reader")
        }.asCoroutineDispatcher()

    @After
    fun closeDispatcher() {
        readerDispatcher.close()
    }

    @Test
    fun `accepted reads are copied and split into exact 640 byte frames`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 700)
        val writer = RecordingPcmWriter()
        val pcm = ByteArray(1_280) { index -> (index % 251).toByte() }
        backend.enqueueData(pcm.copyOfRange(0, 100))
        backend.enqueueData(pcm.copyOfRange(100, 800))
        backend.enqueueData(pcm.copyOfRange(800, 1_280))
        val session = session(backend, writer)

        session.start()
        waitUntil { writer.bytesWritten == pcm.size.toLong() }
        session.stop()
        val frames = session.frames.toList()

        assertEquals(3, writer.writeCalls.size)
        writer.writeCalls.forEach { call ->
            assertNotSame(backend.readBuffers.single(), call.reference)
        }
        assertNotSame(writer.writeCalls[0].reference, writer.writeCalls[1].reference)
        assertEquals(2, frames.size)
        assertArrayEquals(pcm.copyOfRange(0, 640), frames[0].bytes)
        assertArrayEquals(pcm.copyOfRange(640, 1_280), frames[1].bytes)
        assertEquals(listOf(640, 640), frames.map { it.bytes.size })
        session.close()
    }

    @Test
    fun `WAV write completes before a frame can be delivered`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        backend.enqueueData(ByteArray(640) { 7 })
        val writer = RecordingPcmWriter(blockWrites = true)
        val session = session(backend, writer)
        val frame = async(Dispatchers.Default, start = CoroutineStart.UNDISPATCHED) {
            session.frames.first()
        }

        session.start()
        assertTrue(writer.writeEntered.await(5, TimeUnit.SECONDS))
        Thread.sleep(100)
        assertFalse(frame.isCompleted)

        writer.releaseWrites.countDown()
        assertEquals(640, withTimeout(5_000) { frame.await() }.bytes.size)
        session.stop()
        session.close()
    }

    @Test
    fun `trailing PCM shorter than 640 bytes stays in WAV and is not emitted`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 960)
        val writer = RecordingPcmWriter()
        backend.enqueueData(ByteArray(960) { index -> index.toByte() })
        val session = session(backend, writer)

        session.start()
        waitUntil { writer.bytesWritten == 960L }
        session.stop()
        val frames = session.frames.toList()

        assertEquals(960L, writer.bytesWritten)
        assertEquals(1, frames.size)
        assertEquals(640, frames.single().bytes.size)
        session.close()
    }

    @Test
    fun `the 101st queued frame overflows once while later PCM still reaches WAV`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 101 * 640)
        val writer = RecordingPcmWriter()
        backend.enqueueData(ByteArray(101 * 640) { 1 })
        backend.enqueueData(ByteArray(640) { 2 })
        val session = session(backend, writer)

        session.start()
        waitUntil { writer.bytesWritten == 102L * 640L }
        val queuedFrames = withTimeout(5_000) { session.frames.toList() }

        assertEquals(100, queuedFrames.size)
        assertTrue(queuedFrames.all { it.bytes.size == 640 })
        assertEquals(102L * 640L, writer.bytesWritten)
        session.stop()
        assertEquals(
            listOf(MicrophoneCaptureEvent.StreamingQueueOverflow),
            session.events.toList(),
        )
        session.close()
    }

    @Test
    fun `storage failure emits one fatal and stops capture without a frame`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter(failWriteAtCall = 1)
        backend.enqueueData(ByteArray(640) { 1 })
        backend.enqueueData(ByteArray(640) { 2 })
        val session = session(backend, writer)

        session.start()
        waitUntil { backend.stopCalls.get() == 1 }
        session.stop()

        assertEquals(1, writer.writeAttempts.get())
        assertEquals(emptyList<Any>(), session.frames.toList())
        assertEquals(
            listOf(
                MicrophoneCaptureEvent.Fatal(MicrophoneCaptureFailureKind.STORAGE_WRITE),
            ),
            session.events.toList(),
        )
        assertEquals(1, backend.stopCalls.get())
        session.close()
    }

    @Test
    fun `audio read failure emits one fatal and preserves prior WAV bytes`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter()
        backend.enqueueData(ByteArray(640) { 3 })
        backend.enqueueFailure(IOException("microphone disconnected"))
        val session = session(backend, writer)

        session.start()
        waitUntil { backend.stopCalls.get() == 1 }
        session.stop()

        assertEquals(640L, writer.bytesWritten)
        assertEquals(1, session.frames.toList().size)
        assertEquals(
            listOf(
                MicrophoneCaptureEvent.Fatal(MicrophoneCaptureFailureKind.AUDIO_READ),
            ),
            session.events.toList(),
        )
        assertEquals(1, backend.stopCalls.get())
        session.close()
    }

    @Test
    fun `stop unblocks a read from another thread without emitting a fatal`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter()
        val session = session(backend, writer)
        session.start()
        waitUntil { backend.activeReads.get() == 1 }

        withTimeout(5_000) { session.stop() }

        assertEquals(0, backend.activeReads.get())
        assertEquals(1, backend.stopCalls.get())
        assertEquals(1, backend.closeCalls.get())
        assertTrue(backend.lifecycleCalls.indexOf("stop") < backend.lifecycleCalls.indexOf("read-exit"))
        assertTrue(backend.lifecycleCalls.indexOf("read-exit") < backend.lifecycleCalls.indexOf("close"))
        assertNotSame(backend.readerThread, backend.stopThread)
        assertNotSame(backend.readerThread, backend.closeThread)
        assertEquals(emptyList<Any>(), session.events.toList())
        session.close()
    }

    @Test
    fun `adapter-owned input monitoring reaches the public session event flow`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val session = session(backend, RecordingPcmWriter())
        val input = MicrophoneCaptureEvent.Input(AudioInputEvent.Silenced)

        session.start()
        backend.emitInput(AudioInputEvent.Silenced)
        assertEquals(input, withTimeout(5_000) { session.events.first() })
        session.stop()
        session.close()
    }

    @Test
    fun `immediate stop on the reader dispatcher uses separate control and terminates`() =
        runBlocking {
            val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
            val writer = RecordingPcmWriter()
            val session = session(backend, writer)
            val callerThread = AtomicReference<Thread>()

            withTimeout(5_000) {
                withContext(readerDispatcher) {
                    callerThread.set(Thread.currentThread())
                    session.start()
                    session.stop()
                }
            }

            assertEquals(0, backend.activeReads.get())
            assertEquals(0, writer.writeAttempts.get())
            assertNotSame(callerThread.get(), backend.stopThread)
            assertNotSame(callerThread.get(), backend.closeThread)
            assertSame(backend.stopThread, backend.closeThread)
            val controlThread = checkNotNull(backend.stopThread)
            waitUntil { !controlThread.isAlive }
            session.close()
        }

    @Test
    fun `immediate close on the reader dispatcher cannot deadlock a queued reader`() {
        val executor = Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "same-dispatcher-close").apply { isDaemon = true }
        }
        val dispatcher = executor.asCoroutineDispatcher()
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter()
        val session = AndroidMicrophoneCaptureSession(
            backend = backend,
            writer = writer,
            readerDispatcher = dispatcher,
        )
        val future = executor.submit {
            runBlocking { session.start() }
            session.close()
        }

        try {
            future.get(2, TimeUnit.SECONDS)
        } catch (error: TimeoutException) {
            throw AssertionError("close deadlocked on the reader dispatcher", error)
        } finally {
            future.cancel(true)
            dispatcher.close()
        }

        assertEquals(0, backend.activeReads.get())
        assertEquals(0, writer.writeAttempts.get())
        assertEquals(1, backend.stopCalls.get())
        assertEquals(1, backend.closeCalls.get())
        assertEquals(1, writer.closeCalls.get())
        assertNotSame(backend.stopThread, backend.readerThread)
        assertNotSame(backend.closeThread, backend.readerThread)
    }

    @Test
    fun `fatal cleanup leaves the reader thread for backend control`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter()
        backend.enqueueFailure(IOException("microphone disconnected"))
        val session = session(backend, writer)

        session.start()
        waitUntil { backend.stopCalls.get() == 1 }
        session.stop()

        assertEquals(0, writer.writeAttempts.get())
        assertNotSame(backend.readerThread, backend.stopThread)
        assertNotSame(backend.readerThread, backend.closeThread)
        assertSame(backend.stopThread, backend.closeThread)
        assertEquals(
            listOf(
                MicrophoneCaptureEvent.Fatal(MicrophoneCaptureFailureKind.AUDIO_READ),
            ),
            session.events.toList(),
        )
        session.close()
    }

    @Test
    fun `repeated stop terminates backend only once`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val session = session(backend, RecordingPcmWriter())
        session.start()
        waitUntil { backend.activeReads.get() == 1 }

        session.stop()
        session.stop()

        assertEquals(1, backend.stopCalls.get())
        assertEquals(1, backend.closeCalls.get())
        session.close()
    }

    @Test
    fun `stop failure still joins the reader before throwing`() = runBlocking {
        val stopFailure = IOException("backend stop failed")
        val backend = ScriptedAudioCaptureBackend(
            bufferSizeBytes = 640,
            stopFailure = stopFailure,
        )
        val writer = RecordingPcmWriter()
        val session = session(backend, writer)
        session.start()
        waitUntil { backend.activeReads.get() == 1 }

        val thrown = assertThrows(IOException::class.java) {
            runBlocking { session.stop() }
        }
        val writeAttemptsAfterStop = writer.writeAttempts.get()

        assertEquals(stopFailure.message, thrown.message)
        assertEquals(0, backend.activeReads.get())
        assertEquals(1, backend.closeCalls.get())
        assertTrue(backend.lifecycleCalls.indexOf("stop") < backend.lifecycleCalls.indexOf("read-exit"))
        assertTrue(backend.lifecycleCalls.indexOf("read-exit") < backend.lifecycleCalls.indexOf("close"))
        backend.enqueueData(ByteArray(640))
        delay(100)
        assertEquals(writeAttemptsAfterStop, writer.writeAttempts.get())
        assertEquals(CapturedAudio(0, 0), session.finish())
        session.close()
    }

    @Test
    fun `finish is idempotent and only valid after terminal stop`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter(
            finishedAudio = CapturedAudio(durationMillis = 20, bytesWritten = 640),
        )
        val session = session(backend, writer)

        assertThrows(IllegalStateException::class.java) {
            runBlocking { session.finish() }
        }
        session.start()
        waitUntil { backend.activeReads.get() == 1 }
        session.stop()
        val first = session.finish()
        val second = session.finish()

        assertSame(first, second)
        assertEquals(1, writer.finishCalls.get())
        session.close()
    }

    @Test
    fun `close is terminal and idempotent`() = runBlocking {
        val backend = ScriptedAudioCaptureBackend(bufferSizeBytes = 640)
        val writer = RecordingPcmWriter()
        val session = session(backend, writer)
        session.start()
        waitUntil { backend.activeReads.get() == 1 }

        session.close()
        session.close()

        assertEquals(0, backend.activeReads.get())
        assertEquals(1, backend.stopCalls.get())
        assertEquals(1, backend.closeCalls.get())
        assertEquals(1, writer.closeCalls.get())
    }

    private fun session(
        backend: ScriptedAudioCaptureBackend,
        writer: RecordingPcmWriter,
    ) = AndroidMicrophoneCaptureSession(
        backend = backend,
        writer = writer,
        readerDispatcher = readerDispatcher,
    )

    private suspend fun waitUntil(condition: () -> Boolean) {
        withTimeout(5_000) {
            while (!condition()) {
                delay(5)
            }
        }
    }
}

private sealed interface ReadAction {
    data class Data(val bytes: ByteArray) : ReadAction

    data class Failure(val error: IOException) : ReadAction

    data object End : ReadAction
}

private class ScriptedAudioCaptureBackend(
    override val bufferSizeBytes: Int,
    private val stopFailure: IOException? = null,
    private val stopUnblocks: Boolean = true,
) : AudioCaptureBackend, AudioInputMonitoringBackend {
    private val actions = LinkedBlockingQueue<ReadAction>()
    val activeReads = AtomicInteger()
    val stopCalls = AtomicInteger()
    val closeCalls = AtomicInteger()
    val readBuffers = Collections.synchronizedSet(mutableSetOf<ByteArray>())
    val lifecycleCalls = Collections.synchronizedList(mutableListOf<String>())
    private var inputListener: AudioInputEventListener? = null
    @Volatile var readerThread: Thread? = null
        private set
    @Volatile var stopThread: Thread? = null
        private set
    @Volatile var closeThread: Thread? = null
        private set
    private val startCalls = AtomicInteger()

    fun enqueueData(bytes: ByteArray) {
        actions.put(ReadAction.Data(bytes.copyOf()))
    }

    fun enqueueFailure(error: IOException) {
        actions.put(ReadAction.Failure(error))
    }

    fun emitInput(event: AudioInputEvent) {
        inputListener?.onInputEvent(event)
    }

    override fun setInputEventListener(listener: AudioInputEventListener?) {
        inputListener = listener
    }

    override fun start() {
        check(startCalls.incrementAndGet() == 1)
    }

    override fun read(buffer: ByteArray): Int {
        readerThread = Thread.currentThread()
        readBuffers += buffer
        activeReads.incrementAndGet()
        return try {
            when (val action = actions.take()) {
                is ReadAction.Data -> {
                    require(action.bytes.size <= buffer.size)
                    action.bytes.copyInto(buffer)
                    action.bytes.size
                }
                is ReadAction.Failure -> throw action.error
                ReadAction.End -> -1
            }
        } finally {
            activeReads.decrementAndGet()
            lifecycleCalls += "read-exit"
        }
    }

    override fun stop() {
        lifecycleCalls += "stop"
        stopThread = Thread.currentThread()
        stopCalls.incrementAndGet()
        if (stopUnblocks) {
            actions.offer(ReadAction.End)
        }
        stopFailure?.let { throw it }
    }

    override fun close() {
        lifecycleCalls += "close"
        closeThread = Thread.currentThread()
        if (closeCalls.incrementAndGet() == 1) {
            actions.offer(ReadAction.End)
        }
    }
}

private data class WriteCall(
    val reference: ByteArray,
    val bytes: ByteArray,
)

private class RecordingPcmWriter(
    private val blockWrites: Boolean = false,
    private val failWriteAtCall: Int? = null,
    private val finishedAudio: CapturedAudio? = null,
) : PcmWriter {
    val writeCalls = Collections.synchronizedList(mutableListOf<WriteCall>())
    val writeAttempts = AtomicInteger()
    val finishCalls = AtomicInteger()
    val closeCalls = AtomicInteger()
    val writeEntered = CountDownLatch(1)
    val releaseWrites = CountDownLatch(if (blockWrites) 1 else 0)
    private val writtenBytes = AtomicLong()

    val bytesWritten: Long
        get() = writtenBytes.get()

    override fun write(bytes: ByteArray, offset: Int, length: Int) {
        val attempt = writeAttempts.incrementAndGet()
        writeEntered.countDown()
        releaseWrites.await(5, TimeUnit.SECONDS)
        if (attempt == failWriteAtCall) {
            throw IOException("disk full")
        }
        writeCalls += WriteCall(
            reference = bytes,
            bytes = bytes.copyOfRange(offset, offset + length),
        )
        writtenBytes.addAndGet(length.toLong())
    }

    override fun finish(): CapturedAudio {
        finishCalls.incrementAndGet()
        return finishedAudio ?: CapturedAudio(
            durationMillis = writtenBytes.get() * 1_000 / 32_000,
            bytesWritten = writtenBytes.get(),
        )
    }

    override fun close() {
        closeCalls.incrementAndGet()
    }
}
