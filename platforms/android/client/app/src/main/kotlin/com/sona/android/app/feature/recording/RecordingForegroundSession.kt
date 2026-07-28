package com.sona.android.app.feature.recording

import com.sona.android.application.recording.LiveRecordingController
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.StreamingStatus
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch

internal enum class RecordingNotificationPhase {
    PREPARING,
    RECORDING,
    AUDIO_ONLY,
    STOPPING,
}

internal class RecordingForegroundSession(
    private val controller: LiveRecordingController,
    private val scope: CoroutineScope,
    private val onPhaseChanged: (RecordingNotificationPhase) -> Unit,
    private val onFailure: () -> Unit,
    private val onFinished: () -> Unit,
) {
    private var commandReceived = false
    private var commandsInFlight = 0
    private var finished = false
    private var startRequested = false
    private var stopRequested = false
    private var publishedPhase: RecordingNotificationPhase? = null

    init {
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            controller.state.collect(::handleState)
        }
    }

    fun start() {
        if (finished || startRequested) return
        startRequested = true
        commandReceived = true
        val currentPhase = controller.state.value.notificationPhase()
        if (currentPhase != null) {
            publish(currentPhase)
            return
        }

        publish(RecordingNotificationPhase.PREPARING)
        runCommand(controller::start)
    }

    fun stop() {
        if (finished || stopRequested) return
        stopRequested = true
        commandReceived = true
        if (controller.state.value.notificationPhase() == null && commandsInFlight == 0) {
            finish()
            return
        }

        publish(RecordingNotificationPhase.STOPPING)
        runCommand(controller::stop)
    }

    private fun runCommand(command: suspend () -> Unit) {
        commandsInFlight += 1
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            try {
                command()
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                onFailure()
                finish()
            } finally {
                commandsInFlight -= 1
            }
            finishIfTerminal(controller.state.value)
        }
    }

    private fun handleState(state: LiveRecordingState) {
        if (!commandReceived || finished) return
        val phase = state.notificationPhase()
        if (phase != null) {
            publish(phase)
        } else if (state !is LiveRecordingState.Idle || commandsInFlight == 0) {
            finish()
        }
    }

    private fun finishIfTerminal(state: LiveRecordingState) {
        if (state.notificationPhase() == null) finish()
    }

    private fun publish(phase: RecordingNotificationPhase) {
        if (phase == publishedPhase || finished) return
        publishedPhase = phase
        onPhaseChanged(phase)
    }

    private fun finish() {
        if (finished) return
        finished = true
        onFinished()
    }
}

private fun LiveRecordingState.notificationPhase(): RecordingNotificationPhase? = when (this) {
    is LiveRecordingState.Preparing -> RecordingNotificationPhase.PREPARING
    is LiveRecordingState.Recording -> when (streamingStatus) {
        StreamingStatus.Connected -> RecordingNotificationPhase.RECORDING
        is StreamingStatus.AudioOnly -> RecordingNotificationPhase.AUDIO_ONLY
    }
    is LiveRecordingState.Stopping -> RecordingNotificationPhase.STOPPING
    LiveRecordingState.Idle,
    LiveRecordingState.NeedsConfiguration,
    is LiveRecordingState.Completed,
    is LiveRecordingState.Failed,
    -> null
}
