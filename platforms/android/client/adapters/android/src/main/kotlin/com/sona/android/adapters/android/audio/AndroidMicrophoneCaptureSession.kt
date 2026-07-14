package com.sona.android.adapters.android.audio

import com.sona.android.adapters.android.wav.PcmWriter
import com.sona.android.application.recording.CapturedAudio
import com.sona.android.application.recording.MicrophoneCaptureEvent
import com.sona.android.application.recording.MicrophoneCaptureFailureKind
import com.sona.android.application.recording.MicrophoneCaptureSession
import com.sona.android.application.recording.Pcm16Frame
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.coroutines.EmptyCoroutineContext
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.ExecutorCoroutineDispatcher
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

class AndroidMicrophoneCaptureSession internal constructor(
    private val backend: AudioCaptureBackend,
    private val writer: PcmWriter,
    readerDispatcher: CoroutineDispatcher,
    private val controlContext: BackendControlContext,
) : MicrophoneCaptureSession {
    constructor(
        backend: AudioCaptureBackend,
        writer: PcmWriter,
        readerDispatcher: CoroutineDispatcher,
    ) : this(
        backend = backend,
        writer = writer,
        readerDispatcher = readerDispatcher,
        controlContext = OwnedBackendControlContext.create(),
    )

    private enum class State {
        NEW,
        STARTED,
        STOPPING,
        STOPPED,
        CLOSED,
    }

    private val lifecycleMutex = Mutex()
    private val backendLifecycleLock = Any()
    private val readerDispatcher = readerDispatcher
    private val frameChannel = Channel<Pcm16Frame>(capacity = FRAME_QUEUE_CAPACITY)
    private val eventChannel = Channel<MicrophoneCaptureEvent>(capacity = Channel.UNLIMITED)
    private val outputsClosed = AtomicBoolean()
    private val fatalEmitted = AtomicBoolean()
    private val overflowEmitted = AtomicBoolean()
    private val controlContextClosed = AtomicBoolean()

    override val frames: Flow<Pcm16Frame> = frameChannel.receiveAsFlow()
    override val events: Flow<MicrophoneCaptureEvent> = eventChannel.receiveAsFlow()

    @Volatile private var stopRequested = false
    private var state = State.NEW
    private var readerTask: ReaderTask? = null
    private var backendStarted = false
    private var backendStopAttempted = false
    private var backendStopFailure: Throwable? = null
    private var backendClosed = false
    private var backendCloseFailure: Throwable? = null
    private var frameDeliveryEnabled = true
    private var finishedAudio: CapturedAudio? = null
    private var closeCompleted = false

    init {
        require(backend.bufferSizeBytes > 0) { "Audio capture buffer size must be positive." }
        (backend as? AudioInputMonitoringBackend)?.setInputEventListener { event ->
            eventChannel.trySend(MicrophoneCaptureEvent.Input(event))
        }
    }

    override suspend fun start() {
        lifecycleMutex.withLock {
            check(state == State.NEW) { "Microphone capture session has already started." }
            backend.start()
            synchronized(backendLifecycleLock) {
                backendStarted = true
            }
            state = State.STARTED
            readerTask = ReaderTask(readerDispatcher) { pumpAudio() }.also { it.start() }
        }
    }

    override suspend fun stop(): Unit = withContext(NonCancellable) {
        var failure: Throwable? = null
        lifecycleMutex.withLock {
            if (state == State.STOPPED || state == State.CLOSED) {
                return@withLock
            }

            stopRequested = true
            state = State.STOPPING
            readerTask?.cancelIfQueued()
            failure = stopBackend()
            readerTask?.join()
            closeBackend()?.let { closeFailure ->
                if (failure == null) {
                    failure = closeFailure
                }
            }
            (backend as? AudioInputMonitoringBackend)?.setInputEventListener(null)
            closeOutputs()
            closeControlContext()?.let { closeFailure ->
                if (failure == null) {
                    failure = closeFailure
                }
            }
            state = State.STOPPED
        }
        failure?.let { throw it }
        Unit
    }

    override suspend fun finish(): CapturedAudio = lifecycleMutex.withLock {
        finishedAudio?.let { return@withLock it }
        check(state == State.STOPPED) { "Microphone capture must stop before finish." }
        writer.finish().also { finishedAudio = it }
    }

    @Synchronized
    override fun close() {
        if (closeCompleted) {
            return
        }

        var failure: Throwable? = null
        try {
            runBlocking { stop() }
        } catch (error: Throwable) {
            failure = error
        }
        try {
            writer.close()
        } catch (error: Throwable) {
            if (failure == null) {
                failure = error
            }
        }
        runBlocking {
            lifecycleMutex.withLock {
                state = State.CLOSED
            }
        }
        closeCompleted = true
        failure?.let { throw it }
    }

    private suspend fun pumpAudio() {
        val readBuffer = ByteArray(backend.bufferSizeBytes)
        val assembler = PcmFrameAssembler(FRAME_SIZE_BYTES)
        try {
            while (!stopRequested) {
                val bytesRead = try {
                    backend.read(readBuffer)
                } catch (_: Exception) {
                    if (!stopRequested) {
                        failCapture(MicrophoneCaptureFailureKind.AUDIO_READ)
                    }
                    break
                }

                if (bytesRead < 0 || bytesRead > readBuffer.size) {
                    if (!stopRequested) {
                        failCapture(MicrophoneCaptureFailureKind.AUDIO_READ)
                    }
                    break
                }
                if (bytesRead == 0) {
                    continue
                }

                val acceptedBytes = readBuffer.copyOf(bytesRead)
                try {
                    writer.write(acceptedBytes, 0, acceptedBytes.size)
                } catch (_: Exception) {
                    failCapture(MicrophoneCaptureFailureKind.STORAGE_WRITE)
                    break
                }

                if (frameDeliveryEnabled) {
                    for (frame in assembler.append(acceptedBytes, 0, acceptedBytes.size)) {
                        if (frameChannel.trySend(Pcm16Frame(frame)).isFailure) {
                            disableFrameDeliveryForOverflow()
                            break
                        }
                    }
                }
                if (stopRequested) {
                    break
                }
            }
        } finally {
            closeOutputs()
        }
    }

    private fun disableFrameDeliveryForOverflow() {
        frameDeliveryEnabled = false
        if (overflowEmitted.compareAndSet(false, true)) {
            eventChannel.trySend(MicrophoneCaptureEvent.StreamingQueueOverflow)
        }
        frameChannel.close()
    }

    private suspend fun failCapture(kind: MicrophoneCaptureFailureKind) {
        if (fatalEmitted.compareAndSet(false, true)) {
            eventChannel.trySend(MicrophoneCaptureEvent.Fatal(kind))
        }
        stopRequested = true
        stopBackend()
    }

    private suspend fun stopBackend(): Throwable? =
        controlContext.execute { stopBackendOnControlThread() }

    private fun stopBackendOnControlThread(): Throwable? = synchronized(backendLifecycleLock) {
        if (backendStopAttempted) {
            return@synchronized backendStopFailure
        }
        backendStopAttempted = true

        var failure: Throwable? = null
        if (backendStarted) {
            try {
                backend.stop()
            } catch (error: Throwable) {
                failure = error
            }
        }
        backendStopFailure = failure
        failure
    }

    private suspend fun closeBackend(): Throwable? =
        controlContext.execute { closeBackendOnControlThread() }

    private fun closeBackendOnControlThread(): Throwable? = synchronized(backendLifecycleLock) {
        if (backendClosed) {
            return@synchronized backendCloseFailure
        }
        backendClosed = true
        backendCloseFailure = try {
            backend.close()
            null
        } catch (error: Throwable) {
            error
        }
        backendCloseFailure
    }

    private fun closeControlContext(): Throwable? {
        if (!controlContextClosed.compareAndSet(false, true)) {
            return null
        }
        return try {
            controlContext.close()
            null
        } catch (error: Throwable) {
            error
        }
    }

    private fun closeOutputs() {
        if (outputsClosed.compareAndSet(false, true)) {
            frameChannel.close()
            eventChannel.close()
        }
    }

    private companion object {
        const val FRAME_SIZE_BYTES = 640
        const val FRAME_QUEUE_CAPACITY = 100
    }
}

private class ReaderTask(
    private val dispatcher: CoroutineDispatcher,
    private val block: suspend () -> Unit,
) {
    private enum class State {
        QUEUED,
        RUNNING,
        CANCELLED,
        COMPLETED,
    }

    private val state = AtomicReference(State.QUEUED)
    private val completion = CompletableDeferred<Unit>()

    fun start() {
        try {
            dispatcher.dispatch(EmptyCoroutineContext) {
                if (!state.compareAndSet(State.QUEUED, State.RUNNING)) {
                    return@dispatch
                }
                try {
                    runBlocking { block() }
                } finally {
                    state.set(State.COMPLETED)
                    completion.complete(Unit)
                }
            }
        } catch (error: Throwable) {
            state.set(State.COMPLETED)
            completion.complete(Unit)
            throw error
        }
    }

    fun cancelIfQueued() {
        if (state.compareAndSet(State.QUEUED, State.CANCELLED)) {
            completion.complete(Unit)
        }
    }

    suspend fun join() {
        completion.await()
    }
}

internal interface BackendControlContext : AutoCloseable {
    suspend fun <T> execute(block: () -> T): T
}

private class OwnedBackendControlContext(
    private val dispatcher: ExecutorCoroutineDispatcher,
) : BackendControlContext {
    override suspend fun <T> execute(block: () -> T): T =
        withContext(dispatcher + NonCancellable) { block() }

    override fun close() = dispatcher.close()

    companion object {
        private val nextThreadId = AtomicInteger()

        fun create(): OwnedBackendControlContext {
            val dispatcher = Executors.newSingleThreadExecutor { runnable ->
                Thread(
                    runnable,
                    "sona-capture-control-${nextThreadId.incrementAndGet()}",
                ).apply { isDaemon = true }
            }.asCoroutineDispatcher()
            return OwnedBackendControlContext(dispatcher)
        }
    }
}
