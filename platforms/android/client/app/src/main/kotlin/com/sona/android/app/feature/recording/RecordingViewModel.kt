package com.sona.android.app.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.LiveRecordingController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

fun interface LiveRecordingControllerFactory {
    fun create(scope: CoroutineScope): LiveRecordingController
}

class RecordingViewModel(
    controllerFactory: LiveRecordingControllerFactory,
) : ViewModel() {
    private val controller = controllerFactory.create(viewModelScope)
    val state = controller.state

    fun startRecording() {
        viewModelScope.launch { controller.start() }
    }

    fun stopRecording() {
        viewModelScope.launch { controller.stop() }
    }

    fun stopForBackground() {
        stopRecording()
    }

    companion object {
        fun factory(
            controllerFactory: LiveRecordingControllerFactory,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(RecordingViewModel::class.java))
                return RecordingViewModel(controllerFactory) as T
            }
        }
    }
}
