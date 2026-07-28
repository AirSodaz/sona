package com.sona.android.app.feature.recording

import android.content.Context
import androidx.core.content.ContextCompat
import com.sona.android.application.recording.LiveRecordingController
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.RecordingFailure
import com.sona.android.application.recording.RecordingFailureCategory
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.takeWhile
import kotlinx.coroutines.launch

internal interface RecordingServiceCommandLauncher {
    fun startRecording()
    fun stopRecording()
}

internal class AndroidRecordingServiceCommandLauncher(
    context: Context,
) : RecordingServiceCommandLauncher {
    private val appContext = context.applicationContext

    override fun startRecording() {
        ContextCompat.startForegroundService(
            appContext,
            RecordingForegroundService.intent(appContext, RecordingForegroundService.ACTION_START),
        )
    }

    override fun stopRecording() {
        appContext.startService(
            RecordingForegroundService.intent(appContext, RecordingForegroundService.ACTION_STOP),
        )
    }
}

internal class RecordingForegroundGateway(
    private val launcher: RecordingServiceCommandLauncher,
    private val scope: CoroutineScope,
) : LiveRecordingController {
    private val mutableState = MutableStateFlow<LiveRecordingState>(LiveRecordingState.Idle)
    private var stateBridgeJob: Job? = null
    private var attachedSessionState: StateFlow<LiveRecordingState>? = null

    override val state: StateFlow<LiveRecordingState> = mutableState.asStateFlow()

    override suspend fun start() {
        dispatchCommand(launcher::startRecording)
    }

    override suspend fun stop() {
        dispatchCommand(launcher::stopRecording)
    }

    fun attach(sessionState: StateFlow<LiveRecordingState>) {
        stateBridgeJob?.cancel()
        attachedSessionState = sessionState
        stateBridgeJob = scope.launch {
            var activeSessionObserved = false
            try {
                sessionState.takeWhile { state ->
                    mutableState.value = state
                    if (state.isSessionActive()) {
                        activeSessionObserved = true
                    }
                    state.isSessionActive() || !activeSessionObserved
                }.collect()
            } finally {
                if (attachedSessionState === sessionState) {
                    attachedSessionState = null
                    stateBridgeJob = null
                }
            }
        }
    }

    fun detach(sessionState: StateFlow<LiveRecordingState>) {
        if (attachedSessionState !== sessionState) return
        mutableState.value = sessionState.value
        stateBridgeJob?.cancel()
        stateBridgeJob = null
        attachedSessionState = null
    }

    fun reportServiceFailure() {
        mutableState.value = serviceFailure()
    }

    private fun dispatchCommand(command: () -> Unit) {
        try {
            command()
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            reportServiceFailure()
        }
    }

    private fun serviceFailure(): LiveRecordingState.Failed = LiveRecordingState.Failed(
        RecordingFailure(
            category = RecordingFailureCategory.STARTUP,
            message = "Unable to start the recording service.",
        ),
    )
}

private fun LiveRecordingState.isSessionActive(): Boolean =
    this is LiveRecordingState.Preparing ||
        this is LiveRecordingState.Recording ||
        this is LiveRecordingState.Stopping
