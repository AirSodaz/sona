package com.sona.android.app.feature.recording

import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.RecordingFailure
import com.sona.android.application.recording.RecordingFailureCategory
import com.sona.android.application.recording.StreamingStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class RecordingPresentationTest {
    @Test
    fun `timer formats elapsed milliseconds as whole minutes and seconds`() {
        val cases = mapOf(
            -1L to "00:00",
            0L to "00:00",
            999L to "00:00",
            1_000L to "00:01",
            59_999L to "00:59",
            60_000L to "01:00",
            3_661_999L to "61:01",
        )

        cases.forEach { (elapsedMillis, expected) ->
            assertEquals(expected, formatRecordingTimer(elapsedMillis))
        }
    }

    @Test
    fun `start and stop availability follows every domain state`() {
        val cases = listOf(
            AvailabilityCase(LiveRecordingState.Idle, start = true, stop = false),
            AvailabilityCase(LiveRecordingState.NeedsConfiguration, start = true, stop = false),
            AvailabilityCase(LiveRecordingState.Preparing("recording-1"), start = false, stop = false),
            AvailabilityCase(recordingState(), start = false, stop = true),
            AvailabilityCase(LiveRecordingState.Stopping("recording-1"), start = false, stop = false),
            AvailabilityCase(LiveRecordingState.Completed("history-1"), start = true, stop = false),
            AvailabilityCase(
                LiveRecordingState.Failed(failure(RecordingFailureCategory.STARTUP)),
                start = true,
                stop = false,
            ),
        )

        cases.forEach { case ->
            val presentation = case.state.toRecordingPresentation()

            assertEquals("start for ${case.state}", case.start, presentation.isStartAvailable)
            assertEquals("stop for ${case.state}", case.stop, presentation.isStopAvailable)
        }
    }

    @Test
    fun `domain states map to localization categories`() {
        val cases = listOf(
            LiveRecordingState.Idle to RecordingStatusCategory.IDLE,
            LiveRecordingState.NeedsConfiguration to RecordingStatusCategory.NEEDS_CONFIGURATION,
            LiveRecordingState.Preparing("recording-1") to RecordingStatusCategory.PREPARING,
            recordingState() to RecordingStatusCategory.RECORDING,
            LiveRecordingState.Stopping("recording-1") to RecordingStatusCategory.STOPPING,
            LiveRecordingState.Completed("history-1") to RecordingStatusCategory.COMPLETED,
            LiveRecordingState.Completed(
                historyId = "history-1",
                warning = failure(RecordingFailureCategory.AUDIO),
            ) to RecordingStatusCategory.COMPLETED_WITH_WARNING,
        )

        cases.forEach { (state, expected) ->
            assertEquals(expected, state.toRecordingPresentation().statusCategory)
        }
    }

    @Test
    fun `failures map to localized categories without exposing domain messages`() {
        val expectedCategories = mapOf(
            RecordingFailureCategory.INVALID_CONFIGURATION to
                RecordingStatusCategory.INVALID_CONFIGURATION_FAILURE,
            RecordingFailureCategory.STARTUP to RecordingStatusCategory.STARTUP_FAILURE,
            RecordingFailureCategory.AUDIO to RecordingStatusCategory.AUDIO_FAILURE,
            RecordingFailureCategory.STREAMING to RecordingStatusCategory.STREAMING_FAILURE,
            RecordingFailureCategory.PERSISTENCE to RecordingStatusCategory.PERSISTENCE_FAILURE,
        )

        expectedCategories.forEach { (failureCategory, expectedStatusCategory) ->
            val sensitiveMessage = "private diagnostic for $failureCategory"
            val state = LiveRecordingState.Failed(
                RecordingFailure(failureCategory, sensitiveMessage),
            )

            val presentation = state.toRecordingPresentation()

            assertEquals(expectedStatusCategory, presentation.statusCategory)
            assertFalse(presentation.toString().contains(sensitiveMessage))
        }

        val sensitiveWarning = "private completion warning"
        val completedPresentation = LiveRecordingState.Completed(
            historyId = "history-1",
            warning = RecordingFailure(RecordingFailureCategory.AUDIO, sensitiveWarning),
        ).toRecordingPresentation()

        assertEquals(
            RecordingStatusCategory.COMPLETED_WITH_WARNING,
            completedPresentation.statusCategory,
        )
        assertFalse(completedPresentation.toString().contains(sensitiveWarning))
    }

    private data class AvailabilityCase(
        val state: LiveRecordingState,
        val start: Boolean,
        val stop: Boolean,
    )

    private fun recordingState(): LiveRecordingState.Recording =
        LiveRecordingState.Recording(
            recordingId = "recording-1",
            elapsedMillis = 65_000,
            segments = emptyList(),
            streamingStatus = StreamingStatus.Connected,
            inputStatus = AudioInputStatus.Active,
        )

    private fun failure(category: RecordingFailureCategory): RecordingFailure =
        RecordingFailure(category, "domain-only diagnostic")
}
