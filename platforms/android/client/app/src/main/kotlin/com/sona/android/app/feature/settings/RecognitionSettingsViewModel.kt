package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.RecognitionEngine
import com.sona.android.application.recording.RecognitionSettingsPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class RecognitionSettingsUiState(
    val engine: RecognitionEngine = RecognitionEngine.ONLINE,
    val localModel: LocalAsrModel? = null,
    val importInProgress: Boolean = false,
    val importError: Boolean = false,
)

class RecognitionSettingsViewModel(
    private val settingsPort: RecognitionSettingsPort,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(RecognitionSettingsUiState())
    val uiState: StateFlow<RecognitionSettingsUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            settingsPort.settings.collect { settings ->
                mutableUiState.update {
                    it.copy(engine = settings.engine, localModel = settings.localModel)
                }
            }
        }
    }

    fun selectEngine(engine: RecognitionEngine) {
        if (mutableUiState.value.importInProgress) return
        viewModelScope.launch {
            try {
                settingsPort.selectEngine(engine)
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(importError = true) }
            }
        }
    }

    fun importLocalModel(sourceLocation: String) {
        if (sourceLocation.isBlank() || mutableUiState.value.importInProgress) return
        mutableUiState.update { it.copy(importInProgress = true, importError = false) }
        viewModelScope.launch {
            try {
                settingsPort.importLocalModel(sourceLocation)
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(importError = true) }
            } finally {
                mutableUiState.update { it.copy(importInProgress = false) }
            }
        }
    }

    companion object {
        fun factory(settingsPort: RecognitionSettingsPort): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(RecognitionSettingsViewModel::class.java))
                    return RecognitionSettingsViewModel(settingsPort) as T
                }
            }
    }
}
