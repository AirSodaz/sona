package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.BatchCredentialSettingsPort
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineBatchProvider
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class CloudTranscriptionSettingsUiState(
    val selectedProvider: OnlineBatchProvider = OnlineBatchProvider.VOLCENGINE_DOUBAO,
    val configuredProviders: Set<OnlineBatchProvider> = emptySet(),
    val apiKeyInput: String = "",
    val operationInProgress: Boolean = false,
    val operationFailed: Boolean = false,
) {
    val selectedStatus: CredentialStatus
        get() = if (selectedProvider in configuredProviders) {
            CredentialStatus.CONFIGURED
        } else {
            CredentialStatus.NOT_CONFIGURED
        }

    override fun toString(): String =
        "CloudTranscriptionSettingsUiState(" +
            "selectedProvider=$selectedProvider, " +
            "configuredProviders=$configuredProviders, " +
            "apiKeyInput=<redacted>, " +
            "operationInProgress=$operationInProgress, " +
            "operationFailed=$operationFailed)"
}

class CloudTranscriptionSettingsViewModel(
    private val settingsPort: BatchCredentialSettingsPort,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(CloudTranscriptionSettingsUiState())
    val uiState: StateFlow<CloudTranscriptionSettingsUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            try {
                settingsPort.configuration.collect { configuration ->
                    mutableUiState.update {
                        it.copy(
                            selectedProvider = configuration.selectedProvider,
                            configuredProviders = configuration.configuredProviders,
                        )
                    }
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(operationFailed = true) }
            }
        }
    }

    fun onApiKeyInputChanged(value: String) {
        mutableUiState.update { it.copy(apiKeyInput = value, operationFailed = false) }
    }

    fun selectProvider(provider: OnlineBatchProvider) {
        val current = mutableUiState.value
        if (current.operationInProgress || current.selectedProvider == provider) {
            return
        }
        // A key typed for one provider must never be offered to another.
        val previousProvider = current.selectedProvider
        mutableUiState.value = current.copy(
            selectedProvider = provider,
            apiKeyInput = "",
            operationInProgress = true,
            operationFailed = false,
        )
        viewModelScope.launch {
            try {
                settingsPort.selectProvider(provider)
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                // The stored selection is unchanged, so the surface must not claim otherwise.
                mutableUiState.update {
                    it.copy(selectedProvider = previousProvider, operationFailed = true)
                }
            } finally {
                mutableUiState.update { it.copy(operationInProgress = false) }
            }
        }
    }

    fun saveApiKey() {
        val current = mutableUiState.value
        if (current.operationInProgress || current.apiKeyInput.isBlank()) {
            return
        }
        mutableUiState.value = current.copy(operationInProgress = true, operationFailed = false)
        viewModelScope.launch {
            try {
                settingsPort.save(
                    current.selectedProvider,
                    OnlineBatchCredential(current.apiKeyInput),
                )
                mutableUiState.update { it.copy(apiKeyInput = "") }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(operationFailed = true) }
            } finally {
                mutableUiState.update { it.copy(operationInProgress = false) }
            }
        }
    }

    fun clearApiKey() {
        val current = mutableUiState.value
        if (current.operationInProgress) {
            return
        }
        mutableUiState.value = current.copy(operationInProgress = true, operationFailed = false)
        viewModelScope.launch {
            try {
                settingsPort.clear(current.selectedProvider)
                mutableUiState.update { it.copy(apiKeyInput = "") }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(operationFailed = true) }
            } finally {
                mutableUiState.update { it.copy(operationInProgress = false) }
            }
        }
    }

    override fun onCleared() {
        mutableUiState.update { it.copy(apiKeyInput = "") }
    }

    companion object {
        fun factory(
            settingsPort: BatchCredentialSettingsPort,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(CloudTranscriptionSettingsViewModel::class.java))
                return CloudTranscriptionSettingsViewModel(settingsPort) as T
            }
        }
    }
}
