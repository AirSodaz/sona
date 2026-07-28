package com.sona.android.app.feature.recording

import com.sona.android.application.recording.LiveRecordingController
import com.sona.android.application.recording.LiveRecordingState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TestWatcher
import org.junit.runner.Description

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingViewModelTest {
    @get:Rule
    val mainDispatcherRule = RecordingMainDispatcherRule()

    @Test
    fun `exposes the process controller state without creating a recording session`() {
        val controller = FakeLiveRecordingController()
        val viewModel = RecordingViewModel(controller)

        assertSame(controller.state, viewModel.state)
    }

    @Test
    fun `start recording delegates to the controller`() = runTest(mainDispatcherRule.dispatcher) {
        val controller = FakeLiveRecordingController()
        val viewModel = RecordingViewModel(controller)

        viewModel.startRecording()
        advanceUntilIdle()

        assertEquals(1, controller.startCalls)
    }

    @Test
    fun `stop recording delegates to the controller`() = runTest(mainDispatcherRule.dispatcher) {
        val controller = FakeLiveRecordingController()
        val viewModel = RecordingViewModel(controller)

        viewModel.stopRecording()
        advanceUntilIdle()

        assertEquals(1, controller.stopCalls)
    }
}

private class FakeLiveRecordingController : LiveRecordingController {
    override val state: StateFlow<LiveRecordingState> = MutableStateFlow(LiveRecordingState.Idle)
    var startCalls = 0
        private set
    var stopCalls = 0
        private set

    override suspend fun start() {
        startCalls += 1
    }

    override suspend fun stop() {
        stopCalls += 1
    }
}

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingMainDispatcherRule(
    val dispatcher: TestDispatcher = StandardTestDispatcher(),
) : TestWatcher() {
    override fun starting(description: Description) {
        Dispatchers.setMain(dispatcher)
    }

    override fun finished(description: Description) {
        Dispatchers.resetMain()
    }
}
