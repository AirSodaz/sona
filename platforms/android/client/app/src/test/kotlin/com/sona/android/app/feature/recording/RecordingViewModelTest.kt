package com.sona.android.app.feature.recording

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.LiveRecordingUseCase
import com.sona.android.application.recording.StreamingStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class RecordingViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `idle action starts through a ViewModel owned recording scope`() = runTest {
        val recording = FakeLiveRecording()
        var ownerScope: CoroutineScope? = null
        val viewModel = RecordingViewModel { scope ->
            ownerScope = scope
            recording
        }

        viewModel.onRecordAction()
        mainDispatcherRule.dispatcher.scheduler.advanceUntilIdle()

        assertEquals(listOf("start"), recording.calls)
        assertNotNull(ownerScope?.coroutineContext?.get(Job))
        assertTrue(viewModel.actionRequiresMicrophonePermission())
    }

    @Test
    fun `recording action stops and does not request permission again`() = runTest {
        val recording = FakeLiveRecording().apply {
            mutableState.value = LiveRecordingState.Recording(
                recordingId = "recording-1",
                elapsedMillis = 1_000,
                segments = emptyList(),
                streamingStatus = StreamingStatus.Connected,
                inputStatus = AudioInputStatus.Active,
            )
        }
        val viewModel = RecordingViewModel { recording }

        viewModel.onRecordAction()
        mainDispatcherRule.dispatcher.scheduler.advanceUntilIdle()

        assertEquals(listOf("stop"), recording.calls)
        assertFalse(viewModel.actionRequiresMicrophonePermission())
    }

    @Test
    fun `preparing and stopping states ignore duplicate actions`() = runTest {
        val recording = FakeLiveRecording()
        val viewModel = RecordingViewModel { recording }

        recording.mutableState.value = LiveRecordingState.Preparing("recording-1")
        viewModel.onRecordAction()
        recording.mutableState.value = LiveRecordingState.Stopping("recording-1")
        viewModel.onRecordAction()
        mainDispatcherRule.dispatcher.scheduler.advanceUntilIdle()

        assertEquals(emptyList<String>(), recording.calls)
    }

    @Test
    fun `app background always forwards stop so startup races are serialized by the use case`() = runTest {
        val recording = FakeLiveRecording()
        val viewModel = RecordingViewModel { recording }

        viewModel.stopForBackground()
        recording.mutableState.value = LiveRecordingState.Preparing("recording-1")
        viewModel.stopForBackground()
        mainDispatcherRule.dispatcher.scheduler.advanceUntilIdle()

        assertEquals(listOf("stop", "stop"), recording.calls)
    }

    @Test
    fun `timer formatting is stable across minute and hour boundaries`() {
        assertEquals("00:00", formatElapsedMillis(-1))
        assertEquals("01:05", formatElapsedMillis(65_999))
        assertEquals("01:01:01", formatElapsedMillis(3_661_000))
    }

    private class FakeLiveRecording : LiveRecordingUseCase {
        val mutableState = MutableStateFlow<LiveRecordingState>(LiveRecordingState.Idle)
        val calls = mutableListOf<String>()

        override val state: StateFlow<LiveRecordingState> = mutableState

        override suspend fun start() {
            calls += "start"
        }

        override suspend fun stop() {
            calls += "stop"
        }
    }
}
