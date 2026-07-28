package com.sona.android.app.feature.recording

import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.RecordingFailureCategory
import com.sona.android.application.recording.StreamingStatus
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingForegroundGatewayTest {
    @Test
    fun `start and stop dispatch service commands`() = runTest {
        val launcher = FakeRecordingServiceCommandLauncher()
        val gateway = RecordingForegroundGateway(launcher, backgroundScope)

        gateway.start()
        gateway.stop()

        assertEquals(1, launcher.startCalls)
        assertEquals(1, launcher.stopCalls)
    }

    @Test
    fun `service launch failure is exposed as a startup failure`() = runTest {
        val launcher = FakeRecordingServiceCommandLauncher(failStart = true)
        val gateway = RecordingForegroundGateway(launcher, backgroundScope)

        gateway.start()

        val failure = gateway.state.value as LiveRecordingState.Failed
        assertEquals(RecordingFailureCategory.STARTUP, failure.failure.category)
    }

    @Test
    fun `service session state is forwarded until its terminal state`() = runTest {
        val sessionState = MutableStateFlow<LiveRecordingState>(LiveRecordingState.Idle)
        val gateway = RecordingForegroundGateway(FakeRecordingServiceCommandLauncher(), backgroundScope)
        gateway.attach(sessionState)
        runCurrent()

        sessionState.value = recordingState()
        runCurrent()
        assertEquals(sessionState.value, gateway.state.value)

        sessionState.value = LiveRecordingState.Completed("history-1")
        runCurrent()
        assertEquals(sessionState.value, gateway.state.value)
    }

    @Test
    fun `a recreated view model observes the same process recording session`() = runTest {
        val sessionState = MutableStateFlow<LiveRecordingState>(LiveRecordingState.Idle)
        val gateway = RecordingForegroundGateway(FakeRecordingServiceCommandLauncher(), backgroundScope)
        val firstViewModel = RecordingViewModel(gateway)
        gateway.attach(sessionState)
        runCurrent()
        sessionState.value = recordingState()
        runCurrent()

        val recreatedViewModel = RecordingViewModel(gateway)

        assertSame(firstViewModel.state, recreatedViewModel.state)
        assertEquals(sessionState.value, recreatedViewModel.state.value)
    }
}

private class FakeRecordingServiceCommandLauncher(
    private val failStart: Boolean = false,
) : RecordingServiceCommandLauncher {
    var startCalls = 0
        private set
    var stopCalls = 0
        private set

    override fun startRecording() {
        startCalls += 1
        if (failStart) error("service launch rejected")
    }

    override fun stopRecording() {
        stopCalls += 1
    }
}

private fun recordingState(): LiveRecordingState.Recording = LiveRecordingState.Recording(
    recordingId = "recording-1",
    elapsedMillis = 1_000,
    segments = emptyList(),
    streamingStatus = StreamingStatus.Connected,
    inputStatus = AudioInputStatus.Active,
)
