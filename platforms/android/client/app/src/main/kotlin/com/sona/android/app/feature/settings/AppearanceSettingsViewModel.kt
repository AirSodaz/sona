package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.settings.AppearanceSettingsPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class AppearanceSettingsUiState(
    val dynamicColorEnabled: Boolean = false,
    val isLoaded: Boolean = false,
    val operationInProgress: Boolean = false,
    val hasError: Boolean = false,
)

class AppearanceSettingsViewModel(
    private val settings: AppearanceSettingsPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(AppearanceSettingsUiState())
    val state: StateFlow<AppearanceSettingsUiState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            settings.settings
                .catch { error ->
                    if (error is CancellationException) throw error
                    mutableState.update { it.copy(hasError = true) }
                }
                .collect { appearanceSettings ->
                    mutableState.update { current ->
                        if (current.operationInProgress) {
                            current.copy(isLoaded = true)
                        } else {
                            current.copy(
                                dynamicColorEnabled = appearanceSettings.dynamicColorEnabled,
                                isLoaded = true,
                                hasError = false,
                            )
                        }
                    }
                }
        }
    }

    fun setDynamicColorEnabled(enabled: Boolean) {
        val current = mutableState.value
        if (!current.isLoaded || current.operationInProgress) return
        if (current.dynamicColorEnabled == enabled) return

        val previous = current.dynamicColorEnabled
        mutableState.update {
            it.copy(
                dynamicColorEnabled = enabled,
                operationInProgress = true,
                hasError = false,
            )
        }
        viewModelScope.launch {
            try {
                settings.setDynamicColorEnabled(enabled)
                mutableState.update {
                    it.copy(
                        dynamicColorEnabled = enabled,
                        operationInProgress = false,
                    )
                }
            } catch (error: CancellationException) {
                mutableState.update {
                    it.copy(
                        dynamicColorEnabled = previous,
                        operationInProgress = false,
                    )
                }
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(
                        dynamicColorEnabled = previous,
                        operationInProgress = false,
                        hasError = true,
                    )
                }
            }
        }
    }

    companion object {
        fun factory(settings: AppearanceSettingsPort): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(AppearanceSettingsViewModel::class.java))
                    return AppearanceSettingsViewModel(settings) as T
                }
            }
    }
}
