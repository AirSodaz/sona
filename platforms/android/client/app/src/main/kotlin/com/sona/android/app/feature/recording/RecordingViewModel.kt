package com.sona.android.app.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.LiveRecordingUseCase
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

class RecordingViewModel(
    createLiveRecording: (CoroutineScope) -> LiveRecordingUseCase,
) : ViewModel() {
    private val liveRecording = createLiveRecording(viewModelScope)

    val state: StateFlow<LiveRecordingState> = liveRecording.state

    fun onRecordAction() {
        when (state.value) {
            is LiveRecordingState.Recording -> viewModelScope.launch { liveRecording.stop() }
            LiveRecordingState.Idle,
            LiveRecordingState.NeedsConfiguration,
            is LiveRecordingState.Completed,
            is LiveRecordingState.Failed,
            -> viewModelScope.launch { liveRecording.start() }
            is LiveRecordingState.Preparing,
            is LiveRecordingState.Stopping,
            -> Unit
        }
    }

    fun actionRequiresMicrophonePermission(): Boolean = when (state.value) {
        LiveRecordingState.Idle,
        LiveRecordingState.NeedsConfiguration,
        is LiveRecordingState.Completed,
        is LiveRecordingState.Failed,
        -> true
        is LiveRecordingState.Preparing,
        is LiveRecordingState.Recording,
        is LiveRecordingState.Stopping,
        -> false
    }

    fun stopForBackground() {
        viewModelScope.launch { liveRecording.stop() }
    }

    companion object {
        fun factory(
            createLiveRecording: (CoroutineScope) -> LiveRecordingUseCase,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(RecordingViewModel::class.java))
                return RecordingViewModel(createLiveRecording) as T
            }
        }
    }
}
