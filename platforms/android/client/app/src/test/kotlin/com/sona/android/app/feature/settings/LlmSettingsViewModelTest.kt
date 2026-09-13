package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.llm.LlmConfig
import com.sona.android.application.llm.LlmConfigurationPort
import com.sona.android.application.llm.LlmProvider
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class LlmSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    private val defaultProviders = listOf(
        LlmProvider(
            id = "open_ai_compatible",
            aliases = listOf("openai"),
            apiHost = "https://api.openai.com",
            apiPath = "/v1",
            apiVersion = null,
            strategy = "OPEN_AI_COMPATIBLE",
        ),
        LlmProvider(
            id = "anthropic",
            aliases = listOf("claude"),
            apiHost = "https://api.anthropic.com",
            apiPath = "/v1",
            apiVersion = "2023-06-01",
            strategy = "ANTHROPIC",
        ),
        LlmProvider(
            id = "ollama",
            aliases = emptyList(),
            apiHost = "http://127.0.0.1:11434",
            apiPath = "",
            apiVersion = null,
            strategy = "OLLAMA",
        ),
    )

    @Test
    fun `initial state loads configuration, providers, and resolves strategy`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(
                initialProviders = defaultProviders,
                initialConfig = LlmConfig(
                    providerId = "anthropic",
                    strategy = "ANTHROPIC",
                    baseUrl = "https://api.anthropic.com",
                    model = "claude-3-5-sonnet",
                    apiPath = "/v1",
                    apiVersion = "2023-06-01",
                    configured = true,
                ),
                apiKey = "sk-ant-test",
            )

            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            val state = viewModel.state.value
            assertEquals(3, state.providers.size)
            assertEquals("anthropic", state.providerId)
            assertEquals("ANTHROPIC", state.strategy)
            assertEquals("claude-3-5-sonnet", state.model)
            assertEquals("https://api.anthropic.com", state.baseUrl)
            assertEquals("/v1", state.apiPath)
            assertEquals("2023-06-01", state.apiVersion)
            assertTrue(state.hasApiKey)
            assertFalse(state.saving)
            assertFalse(state.saved)
            assertFalse(state.error)
        }

    @Test
    fun `selecting another provider updates providerId, baseUrl, apiPath, apiVersion, and strategy`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(initialProviders = defaultProviders)
            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            viewModel.provider("ollama")
            advanceUntilIdle()

            val state = viewModel.state.value
            assertEquals("ollama", state.providerId)
            assertEquals("OLLAMA", state.strategy)
            assertEquals("http://127.0.0.1:11434", state.baseUrl)
            assertEquals("", state.apiPath)
            assertNull(state.apiVersion.ifBlank { null })
            assertFalse(state.saved)
        }

    @Test
    fun `field changes update state and reset saved status`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(initialProviders = defaultProviders)
            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            viewModel.model("custom-model")
            viewModel.baseUrl("https://custom.host.com")
            viewModel.apiPath("/v2/chat")
            viewModel.apiVersion("2024-01-01")
            viewModel.apiKey("new-key")

            val state = viewModel.state.value
            assertEquals("custom-model", state.model)
            assertEquals("https://custom.host.com", state.baseUrl)
            assertEquals("/v2/chat", state.apiPath)
            assertEquals("2024-01-01", state.apiVersion)
            assertFalse(state.saved)
        }

    @Test
    fun `saving valid configuration passes current dynamic strategy and updates state`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(
                initialProviders = defaultProviders,
                apiKey = null,
            )
            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            viewModel.provider("anthropic")
            viewModel.model("claude-3-haiku")
            viewModel.apiKey("secret-anthropic-key")

            viewModel.save()
            advanceUntilIdle()

            val state = viewModel.state.value
            assertFalse(state.saving)
            assertTrue(state.saved)
            assertTrue(state.hasApiKey)
            assertFalse(state.error)

            val saved = repository.savedConfig
            assertEquals("anthropic", saved?.providerId)
            assertEquals("ANTHROPIC", saved?.strategy)
            assertEquals("https://api.anthropic.com", saved?.baseUrl)
            assertEquals("claude-3-haiku", saved?.model)
            assertEquals("secret-anthropic-key", repository.savedApiKey)
        }

    @Test
    fun `save with blank model or blank baseUrl or missing apiKey sets error`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(
                initialProviders = defaultProviders,
                apiKey = null,
            )
            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            viewModel.model("")
            viewModel.save()
            advanceUntilIdle()

            assertTrue(viewModel.state.value.error)
            assertNull(repository.savedConfig)
        }

    @Test
    fun `repository save failure sets error flag and resets saving`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(
                initialProviders = defaultProviders,
                apiKey = "existing-key",
            ).apply { shouldFailSave = true }

            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            viewModel.save()
            advanceUntilIdle()

            val state = viewModel.state.value
            assertFalse(state.saving)
            assertTrue(state.error)
            assertFalse(state.saved)
        }

    @Test
    fun `clear invokes repository clear and resets ui state while keeping providers`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(
                initialProviders = defaultProviders,
                apiKey = "stored-api-key",
            )
            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()
            assertTrue(viewModel.state.value.hasApiKey)

            viewModel.clear()
            advanceUntilIdle()

            assertEquals(1, repository.clearCalls)
            val state = viewModel.state.value
            assertFalse(state.hasApiKey)
            assertFalse(state.saved)
            assertEquals(3, state.providers.size)
            assertEquals("open_ai_compatible", state.providerId)
            assertEquals("OPEN_AI_COMPATIBLE", state.strategy)
        }

    @Test
    fun `subsequent repository configuration emission updates strategy and fields`() =
        runTest(mainDispatcherRule.dispatcher) {
            val repository = FakeLlmConfigurationPort(initialProviders = defaultProviders)
            val viewModel = LlmSettingsViewModel(repository)
            advanceUntilIdle()

            repository.configFlow.value = LlmConfig(
                providerId = "ollama",
                strategy = "OLLAMA",
                baseUrl = "http://192.168.1.100:11434",
                model = "llama3.2",
                configured = true,
            )
            advanceUntilIdle()

            val state = viewModel.state.value
            assertEquals("ollama", state.providerId)
            assertEquals("OLLAMA", state.strategy)
            assertEquals("http://192.168.1.100:11434", state.baseUrl)
            assertEquals("llama3.2", state.model)
            assertTrue(state.hasApiKey)
        }
}

private class FakeLlmConfigurationPort(
    initialProviders: List<LlmProvider> = emptyList(),
    initialConfig: LlmConfig = LlmConfig(),
    var apiKey: String? = null,
) : LlmConfigurationPort {
    val providersFlow = MutableStateFlow(initialProviders)
    val configFlow = MutableStateFlow(initialConfig)

    override val providers: Flow<List<LlmProvider>> = providersFlow
    override val configuration: Flow<LlmConfig> = configFlow

    var savedConfig: LlmConfig? = null
    var savedApiKey: String? = null
    var clearCalls = 0
    var shouldFailSave = false

    override suspend fun save(config: LlmConfig, apiKey: String) {
        if (shouldFailSave) error("Save failed")
        savedConfig = config
        savedApiKey = apiKey
        this.apiKey = apiKey
        configFlow.value = config
    }

    override suspend fun loadApiKey(): String? = apiKey

    override suspend fun clear() {
        clearCalls += 1
        apiKey = null
        configFlow.value = LlmConfig()
    }
}
