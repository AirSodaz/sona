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
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class CredentialSettingsUiState(
    val status: CredentialStatus? = null,
    val operationInProgress: Boolean = false,
    val hasError: Boolean = false,
)

class CredentialSettingsViewModel(
    private val settings: StreamingCredentialSettingsPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(CredentialSettingsUiState())
    val state: StateFlow<CredentialSettingsUiState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            settings.status
                .catch { error ->
                    if (error is CancellationException) throw error
                    mutableState.update { it.copy(hasError = true) }
                }
                .collect { status ->
                    mutableState.update { it.copy(status = status, hasError = false) }
                }
        }
    }

    fun save(apiKey: String) {
        if (apiKey.isBlank()) {
            mutableState.update { it.copy(hasError = true) }
            return
        }
        if (mutableState.value.operationInProgress) return
        mutableState.update { it.copy(operationInProgress = true, hasError = false) }
        viewModelScope.launch {
            try {
                settings.save(StreamingCredential(apiKey))
                mutableState.update {
                    it.copy(
                        status = CredentialStatus.CONFIGURED,
                        operationInProgress = false,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(operationInProgress = false, hasError = true)
                }
            }
        }
    }

    fun clear() {
        if (mutableState.value.operationInProgress) return
        mutableState.update { it.copy(operationInProgress = true, hasError = false) }
        viewModelScope.launch {
            try {
                settings.clear()
                mutableState.update {
                    it.copy(
                        status = CredentialStatus.NOT_CONFIGURED,
                        operationInProgress = false,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(operationInProgress = false, hasError = true)
                }
            }
        }
    }

    companion object {
        fun factory(settings: StreamingCredentialSettingsPort): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(CredentialSettingsViewModel::class.java))
                    return CredentialSettingsViewModel(settings) as T
                }
            }
    }
}
