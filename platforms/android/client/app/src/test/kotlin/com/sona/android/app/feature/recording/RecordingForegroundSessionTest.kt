package com.sona.android.app.feature.recording

import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingController
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.RecordingFailure
import com.sona.android.application.recording.RecordingFailureCategory
import com.sona.android.application.recording.StreamingStatus
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingForegroundSessionTest {
    @Test
    fun `foreground notification is published before the controller opens recording`() = runTest {
        val events = mutableListOf<String>()
        val controller = FakeSessionController().apply {
            startAction = {
                events += "controller.start"
                mutableState.value = sessionRecordingState()
            }
        }
        val session = createSession(
            controller = controller,
            onPhaseChanged = { events += "notification.$it" },
        )

        session.start()
        runCurrent()

        assertEquals("notification.PREPARING", events.first())
        assertEquals(1, controller.startCalls)
    }

    @Test
    fun `duplicate start commands are idempotent`() = runTest {
        val controller = FakeSessionController().apply {
            startAction = { mutableState.value = sessionRecordingState() }
        }
        val session = createSession(controller)

        session.start()
        session.start()
        runCurrent()

        assertEquals(1, controller.startCalls)
    }

    @Test
    fun `notification stop waits for persistence before finishing the service`() = runTest {
        val allowPersistence = CompletableDeferred<Unit>()
        var finished = false
        val phases = mutableListOf<RecordingNotificationPhase>()
        val controller = FakeSessionController(sessionRecordingState()).apply {
            stopAction = {
                mutableState.value = LiveRecordingState.Stopping("recording-1")
                allowPersistence.await()
                mutableState.value = LiveRecordingState.Completed("history-1")
            }
        }
        val session = createSession(
            controller = controller,
            onPhaseChanged = phases::add,
            onFinished = { finished = true },
        )
        session.start()

        session.stop()
        runCurrent()

        assertEquals(RecordingNotificationPhase.STOPPING, phases.last())
        assertFalse(finished)

        allowPersistence.complete(Unit)
        runCurrent()

        assertTrue(finished)
        assertEquals(1, controller.stopCalls)
    }

    @Test
    fun `terminal startup failure finishes the service`() = runTest {
        var finished = false
        val controller = FakeSessionController().apply {
            startAction = {
                mutableState.value = LiveRecordingState.Failed(
                    RecordingFailure(
                        category = RecordingFailureCategory.STARTUP,
                        message = "startup failed",
                    ),
                )
            }
        }
        val session = createSession(controller, onFinished = { finished = true })

        session.start()
        runCurrent()

        assertTrue(finished)
    }

    @Test
    fun `controller exception reports service failure and finishes`() = runTest {
        var failures = 0
        var finished = false
        val controller = FakeSessionController().apply {
            startAction = { error("unexpected") }
        }
        val session = createSession(
            controller = controller,
            onFailure = { failures += 1 },
            onFinished = { finished = true },
        )

        session.start()
        runCurrent()

        assertEquals(1, failures)
        assertTrue(finished)
    }

    @Test
    fun `pause command dispatches to controller and updates phase to PAUSED`() = runTest {
        val phases = mutableListOf<RecordingNotificationPhase>()
        val controller = FakeSessionController(sessionRecordingState()).apply {
            pauseAction = {
                mutableState.value = sessionRecordingState().copy(isPaused = true)
            }
        }
        val session = createSession(controller, onPhaseChanged = phases::add)
        session.start()
        runCurrent()

        session.pause()
        runCurrent()

        assertEquals(RecordingNotificationPhase.PAUSED, phases.last())
        assertEquals(1, controller.pauseCalls)
    }

    @Test
    fun `resume command dispatches to controller and updates phase back to RECORDING`() = runTest {
        val phases = mutableListOf<RecordingNotificationPhase>()
        val controller = FakeSessionController(sessionRecordingState().copy(isPaused = true)).apply {
            resumeAction = {
                mutableState.value = sessionRecordingState().copy(isPaused = false)
            }
        }
        val session = createSession(controller, onPhaseChanged = phases::add)
        session.start()
        runCurrent()

        session.resume()
        runCurrent()

        assertEquals(RecordingNotificationPhase.RECORDING, phases.last())
        assertEquals(1, controller.resumeCalls)
    }

    private fun TestScope.createSession(
        controller: LiveRecordingController,
        onPhaseChanged: (RecordingNotificationPhase) -> Unit = {},
        onFailure: () -> Unit = {},
        onFinished: () -> Unit = {},
    ): RecordingForegroundSession = RecordingForegroundSession(
        controller = controller,
        scope = backgroundScope,
        onPhaseChanged = onPhaseChanged,
        onFailure = onFailure,
        onFinished = onFinished,
    )
}

private class FakeSessionController(
    initialState: LiveRecordingState = LiveRecordingState.Idle,
) : LiveRecordingController {
    val mutableState = MutableStateFlow(initialState)
    override val state: StateFlow<LiveRecordingState> = mutableState
    var startCalls = 0
        private set
    var stopCalls = 0
        private set
    var pauseCalls = 0
        private set
    var resumeCalls = 0
        private set
    var startAction: suspend () -> Unit = {}
    var stopAction: suspend () -> Unit = {}
    var pauseAction: suspend () -> Unit = {}
    var resumeAction: suspend () -> Unit = {}

    override suspend fun start() {
        startCalls += 1
        startAction()
    }

    override suspend fun stop() {
        stopCalls += 1
        stopAction()
    }

    override suspend fun pause() {
        pauseCalls += 1
        pauseAction()
    }

    override suspend fun resume() {
        resumeCalls += 1
        resumeAction()
    }
}

private fun sessionRecordingState(): LiveRecordingState.Recording = LiveRecordingState.Recording(
    recordingId = "recording-1",
    elapsedMillis = 1_000,
    segments = emptyList(),
    streamingStatus = StreamingStatus.Connected,
    inputStatus = AudioInputStatus.Active,
)
