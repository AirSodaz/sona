package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDeviceCapabilities
import com.sona.android.application.recording.LocalAsrDeviceCapabilitiesPort
import com.sona.android.application.recording.LocalAsrDownloadProgress
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalAsrModelCatalogPort
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
    val installedModels: List<LocalAsrModel> = emptyList(),
    val catalogModels: List<LocalAsrCatalogModel> = emptyList(),
    val deviceCapabilities: LocalAsrDeviceCapabilities? = null,
    val catalogLoading: Boolean = true,
    val operationModelId: String? = null,
    val downloadProgress: LocalAsrDownloadProgress? = null,
    val operationError: Boolean = false,
    val validationByModelId: Map<String, Boolean> = emptyMap(),
)

class RecognitionSettingsViewModel(
    private val settingsPort: RecognitionSettingsPort,
    private val catalogPort: LocalAsrModelCatalogPort,
    private val deviceCapabilitiesPort: LocalAsrDeviceCapabilitiesPort,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(RecognitionSettingsUiState())
    val uiState: StateFlow<RecognitionSettingsUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            settingsPort.settings.collect { settings ->
                mutableUiState.update {
                    it.copy(
                        engine = settings.engine,
                        localModel = settings.localModel,
                        installedModels = settings.installedModels,
                    )
                }
            }
        }
        loadCatalogAndCapabilities()
    }

    fun selectEngine(engine: RecognitionEngine) = runOperation(null) {
        settingsPort.selectEngine(engine)
    }

    fun selectLocalModel(modelId: String) = runOperation(modelId) {
        settingsPort.selectLocalModel(modelId)
    }

    fun downloadLocalModel(modelId: String) {
        val model = mutableUiState.value.catalogModels.firstOrNull { it.id == modelId } ?: return
        runOperation(modelId, clearProgress = false) {
            settingsPort.downloadLocalModel(
                model,
                LocalAsrDownloadProgressListener { progress ->
                    mutableUiState.update { it.copy(downloadProgress = progress) }
                },
            )
        }
    }

    fun validateLocalModel(modelId: String) = runOperation(modelId) {
        val result = settingsPort.validateLocalModel(modelId)
        mutableUiState.update {
            it.copy(validationByModelId = it.validationByModelId + (modelId to result.valid))
        }
    }

    fun deleteLocalModel(modelId: String) = runOperation(modelId) {
        settingsPort.deleteLocalModel(modelId)
        mutableUiState.update {
            it.copy(validationByModelId = it.validationByModelId - modelId)
        }
    }

    fun refreshCatalog() {
        if (mutableUiState.value.operationModelId != null) return
        loadCatalogAndCapabilities()
    }

    private fun loadCatalogAndCapabilities() {
        mutableUiState.update { it.copy(catalogLoading = true, operationError = false) }
        viewModelScope.launch {
            try {
                val capabilities = deviceCapabilitiesPort.detect()
                val catalog = catalogPort.loadStreamingModels()
                mutableUiState.update {
                    it.copy(
                        catalogModels = catalog,
                        deviceCapabilities = capabilities,
                        catalogLoading = false,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(catalogLoading = false, operationError = true) }
            }
        }
    }

    private fun runOperation(
        modelId: String?,
        clearProgress: Boolean = true,
        operation: suspend () -> Unit,
    ) {
        if (mutableUiState.value.operationModelId != null) return
        mutableUiState.update {
            it.copy(
                operationModelId = modelId ?: ENGINE_OPERATION_ID,
                downloadProgress = if (clearProgress) null else it.downloadProgress,
                operationError = false,
            )
        }
        viewModelScope.launch {
            try {
                operation()
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(operationError = true) }
            } finally {
                mutableUiState.update {
                    it.copy(operationModelId = null, downloadProgress = null)
                }
            }
        }
    }

    companion object {
        private const val ENGINE_OPERATION_ID = "__engine__"

        fun factory(
            settingsPort: RecognitionSettingsPort,
            catalogPort: LocalAsrModelCatalogPort,
            deviceCapabilitiesPort: LocalAsrDeviceCapabilitiesPort,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(RecognitionSettingsViewModel::class.java))
                return RecognitionSettingsViewModel(
                    settingsPort,
                    catalogPort,
                    deviceCapabilitiesPort,
                ) as T
            }
        }
    }
}
