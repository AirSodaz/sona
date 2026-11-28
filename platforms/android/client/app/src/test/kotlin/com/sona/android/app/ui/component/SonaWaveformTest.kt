package com.sona.android.app.ui.component

import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.RecordingFailure
import com.sona.android.application.recording.RecordingFailureCategory
import com.sona.android.application.recording.StreamingStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SonaWaveformTest {

    @Test
    fun `smoothEnvelopeStep attacks faster than it releases`() {
        val dt = 0.016f
        val attacked = smoothEnvelopeStep(current = 0f, target = 1f, dtSeconds = dt)
        val released = smoothEnvelopeStep(current = 1f, target = 0f, dtSeconds = dt)

        val attackDelta = attacked
        val releaseDelta = 1f - released
        assertTrue(
            "Attack delta ($attackDelta) should exceed release delta ($releaseDelta)",
            attackDelta > releaseDelta,
        )
        assertTrue(attacked in 0f..1f)
        assertTrue(released in 0f..1f)
    }

    @Test
    fun `edgeAttenuationWindow tapers cleanly to zero at boundaries and peaks at center`() {
        assertEquals(0f, edgeAttenuationWindow(0f), 0.0001f)
        assertEquals(0f, edgeAttenuationWindow(1f), 0.0001f)
        assertEquals(1f, edgeAttenuationWindow(0.5f, peakCenterFraction = 0.5f), 0.0001f)

        val leftFocusedPeak = edgeAttenuationWindow(0.30f, peakCenterFraction = 0.30f)
        val rightTail = edgeAttenuationWindow(0.80f, peakCenterFraction = 0.30f)
        assertEquals(1f, leftFocusedPeak, 0.001f)
        assertTrue("Left-focused window should taper toward right ($rightTail)", rightTail < 0.6f)
    }

    @Test
    fun `evaluateWaveDisplacement returns bounded non-negative displacements with zero endpoints`() {
        assertEquals(
            0f,
            evaluateWaveDisplacement(
                fraction = 0f,
                cycles = 2.25f,
                phaseRad = 1.2f,
                amplitudePx = 24f,
                peakCenterFraction = 0.3f,
            ),
            0.0001f,
        )
        assertEquals(
            0f,
            evaluateWaveDisplacement(
                fraction = 1f,
                cycles = 2.25f,
                phaseRad = 1.2f,
                amplitudePx = 24f,
                peakCenterFraction = 0.3f,
            ),
            0.0001f,
        )

        val midDisplacement = evaluateWaveDisplacement(
            fraction = 0.3f,
            cycles = 2.25f,
            phaseRad = 1.2f,
            amplitudePx = 24f,
            peakCenterFraction = 0.3f,
        )
        assertTrue("Displacement ($midDisplacement) must be within 0..amplitudePx", midDisplacement in 0f..24f)
    }

    @Test
    fun `recording states map to expected WaveformVisualMode`() {
        assertEquals(WaveformVisualMode.IDLE, LiveRecordingState.Idle.toWaveformVisualMode())
        assertEquals(
            WaveformVisualMode.IDLE,
            LiveRecordingState.NeedsConfiguration.toWaveformVisualMode(),
        )
        assertEquals(
            WaveformVisualMode.IDLE,
            LiveRecordingState.Completed("history-1").toWaveformVisualMode(),
        )
        assertEquals(
            WaveformVisualMode.IDLE,
            LiveRecordingState.Failed(
                RecordingFailure(RecordingFailureCategory.AUDIO, "err"),
            ).toWaveformVisualMode(),
        )
        assertEquals(
            WaveformVisualMode.PREPARING,
            LiveRecordingState.Preparing("rec-1").toWaveformVisualMode(),
        )
        assertEquals(
            WaveformVisualMode.PREPARING,
            LiveRecordingState.Stopping("rec-1").toWaveformVisualMode(),
        )

        val active = LiveRecordingState.Recording(
            recordingId = "rec-1",
            elapsedMillis = 1_000L,
            segments = emptyList(),
            streamingStatus = StreamingStatus.Connected,
            inputStatus = AudioInputStatus.Active,
            isPaused = false,
        )
        assertEquals(WaveformVisualMode.ACTIVE, active.toWaveformVisualMode())
        assertEquals(WaveformVisualMode.PAUSED, active.copy(isPaused = true).toWaveformVisualMode())
    }
}
