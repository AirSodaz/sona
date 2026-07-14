package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.StreamingCredential
import com.sona.android.application.recording.StreamingCredentialSettingsPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class CredentialSettingsUiState(
    val status: CredentialStatus = CredentialStatus.NOT_CONFIGURED,
    val credentialInput: String = "",
    val operationInProgress: Boolean = false,
    val operationFailed: Boolean = false,
) {
    override fun toString(): String =
        "CredentialSettingsUiState(" +
            "status=$status, " +
            "credentialInput=<redacted>, " +
            "operationInProgress=$operationInProgress, " +
            "operationFailed=$operationFailed)"
}

class CredentialSettingsViewModel(
    private val settingsPort: StreamingCredentialSettingsPort,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(CredentialSettingsUiState())
    val uiState: StateFlow<CredentialSettingsUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            try {
                settingsPort.status.collect { status ->
                    mutableUiState.update { it.copy(status = status) }
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(operationFailed = true) }
            }
        }
    }

    fun onCredentialInputChanged(value: String) {
        mutableUiState.update {
            it.copy(credentialInput = value, operationFailed = false)
        }
    }

    fun saveCredential() {
        val current = mutableUiState.value
        if (current.operationInProgress || current.credentialInput.isBlank()) {
            return
        }
        mutableUiState.value = current.copy(
            operationInProgress = true,
            operationFailed = false,
        )
        viewModelScope.launch {
            try {
                settingsPort.save(StreamingCredential(current.credentialInput))
                mutableUiState.update { it.copy(credentialInput = "") }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableUiState.update { it.copy(operationFailed = true) }
            } finally {
                mutableUiState.update { it.copy(operationInProgress = false) }
            }
        }
    }

    fun clearCredential() {
        val current = mutableUiState.value
        if (current.operationInProgress) {
            return
        }
        mutableUiState.value = current.copy(
            operationInProgress = true,
            operationFailed = false,
        )
        viewModelScope.launch {
            try {
                settingsPort.clear()
                mutableUiState.update { it.copy(credentialInput = "") }
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
        mutableUiState.update { it.copy(credentialInput = "") }
    }

    companion object {
        fun factory(
            settingsPort: StreamingCredentialSettingsPort,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(CredentialSettingsViewModel::class.java))
                return CredentialSettingsViewModel(settingsPort) as T
            }
        }
    }
}
