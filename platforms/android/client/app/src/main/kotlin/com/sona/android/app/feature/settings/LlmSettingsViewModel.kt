package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.llm.LlmConfig
import com.sona.android.application.llm.LlmConfigurationPort
import com.sona.android.application.llm.LlmProvider
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class LlmSettingsUiState(
    val providers: List<LlmProvider> = emptyList(),
    val providerId: String = "open_ai_compatible",
    val model: String = "gpt-4o-mini",
    val baseUrl: String = "https://api.openai.com",
    val apiPath: String = "",
    val apiVersion: String = "",
    val hasApiKey: Boolean = false,
    val saving: Boolean = false,
    val saved: Boolean = false,
    val error: Boolean = false,
)

class LlmSettingsViewModel(private val repository: LlmConfigurationPort) : ViewModel() {
    private val mutableState = MutableStateFlow(LlmSettingsUiState())
    val state: StateFlow<LlmSettingsUiState> = mutableState.asStateFlow()
    private var apiKeyDraft = ""

    init {
        viewModelScope.launch { repository.providers.collect { values -> mutableState.update { it.copy(providers = values) } } }
        viewModelScope.launch {
            repository.configuration.collect { config ->
                mutableState.update { it.copy(providerId = config.providerId, model = config.model, baseUrl = config.baseUrl, apiPath = config.apiPath.orEmpty(), apiVersion = config.apiVersion.orEmpty()) }
            }
            mutableState.update { it.copy(hasApiKey = !repository.loadApiKey().isNullOrBlank()) }
        }
    }

    fun provider(value: String) = mutableState.update { it.copy(providerId = value, saved = false) }
    fun model(value: String) = mutableState.update { it.copy(model = value, saved = false) }
    fun baseUrl(value: String) = mutableState.update { it.copy(baseUrl = value, saved = false) }
    fun apiPath(value: String) = mutableState.update { it.copy(apiPath = value, saved = false) }
    fun apiVersion(value: String) = mutableState.update { it.copy(apiVersion = value, saved = false) }
    fun apiKey(value: String) { apiKeyDraft = value; mutableState.update { it.copy(saved = false) } }
    fun save() {
        val current = state.value
        if (current.model.isBlank() || current.baseUrl.isBlank() || (apiKeyDraft.isBlank() && !current.hasApiKey)) {
            mutableState.update { it.copy(error = true) }; return
        }
        viewModelScope.launch {
            mutableState.update { it.copy(saving = true, error = false) }
            runCatching {
                repository.save(LlmConfig(providerId = current.providerId, strategy = "OPEN_AI_COMPATIBLE", baseUrl = current.baseUrl, model = current.model, apiPath = current.apiPath.ifBlank { null }, apiVersion = current.apiVersion.ifBlank { null }, configured = true), apiKeyDraft.ifBlank { repository.loadApiKey().orEmpty() })
            }.onSuccess { apiKeyDraft = ""; mutableState.update { it.copy(saving = false, hasApiKey = true, saved = true) } }
                .onFailure { mutableState.update { it.copy(saving = false, error = true) } }
        }
    }
    fun clear() { viewModelScope.launch { repository.clear(); apiKeyDraft = ""; mutableState.value = LlmSettingsUiState(providers = state.value.providers) } }

    companion object {
        fun factory(repository: LlmConfigurationPort) = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST") override fun <T : ViewModel> create(modelClass: Class<T>): T = LlmSettingsViewModel(repository) as T
        }
    }
}
