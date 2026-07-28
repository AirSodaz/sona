package com.sona.android.app.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.LiveRecordingController
import kotlinx.coroutines.launch

class RecordingViewModel(
    private val controller: LiveRecordingController,
) : ViewModel() {
    val state = controller.state

    fun startRecording() {
        viewModelScope.launch { controller.start() }
    }

    fun stopRecording() {
        viewModelScope.launch { controller.stop() }
    }

    companion object {
        fun factory(
            controller: LiveRecordingController,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(RecordingViewModel::class.java))
                return RecordingViewModel(controller) as T
            }
        }
    }
}
