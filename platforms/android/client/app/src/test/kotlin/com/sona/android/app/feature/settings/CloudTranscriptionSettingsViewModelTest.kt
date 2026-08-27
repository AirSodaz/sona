package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.recording.BatchCredentialConfiguration
import com.sona.android.application.recording.BatchCredentialSettingsPort
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineAsrProvider
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class CloudTranscriptionSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `collects the stored selection and configured providers`() =
        runTest(mainDispatcherRule.dispatcher) {
            val settings = FakeBatchCredentialSettingsPort(
                BatchCredentialConfiguration(
                    selectedProvider = OnlineAsrProvider.GROQ_WHISPER,
                    configuredProviders = setOf(OnlineAsrProvider.GROQ_WHISPER),
                ),
            )

            val viewModel = CloudTranscriptionSettingsViewModel(settings)
            advanceUntilIdle()

            assertEquals(OnlineAsrProvider.GROQ_WHISPER, viewModel.uiState.value.selectedProvider)
            assertEquals(CredentialStatus.CONFIGURED, viewModel.uiState.value.selectedStatus)
        }

    @Test
    fun `save sends the typed key for the selected provider and clears the input`() =
        runTest(mainDispatcherRule.dispatcher) {
            val settings = FakeBatchCredentialSettingsPort(
                BatchCredentialConfiguration(
                    selectedProvider = OnlineAsrProvider.MISTRAL_VOXTRAL,
                ),
            )
            val viewModel = CloudTranscriptionSettingsViewModel(settings)
            advanceUntilIdle()
            viewModel.onApiKeyInputChanged("cloud-api-key")

            viewModel.saveApiKey()
            advanceUntilIdle()

            assertEquals(
                listOf(
                    OnlineAsrProvider.MISTRAL_VOXTRAL to OnlineBatchCredential("cloud-api-key"),
                ),
                settings.savedCredentials,
            )
            assertEquals("", viewModel.uiState.value.apiKeyInput)
            assertFalse(viewModel.uiState.value.operationFailed)
        }

    @Test
    fun `a blank key is never sent`() = runTest(mainDispatcherRule.dispatcher) {
        val settings = FakeBatchCredentialSettingsPort()
        val viewModel = CloudTranscriptionSettingsViewModel(settings)
        viewModel.onApiKeyInputChanged("   ")

        viewModel.saveApiKey()
        advanceUntilIdle()

        assertTrue(settings.savedCredentials.isEmpty())
    }

    @Test
    fun `switching provider persists the selection and drops the typed key`() =
        runTest(mainDispatcherRule.dispatcher) {
            val settings = FakeBatchCredentialSettingsPort()
            val viewModel = CloudTranscriptionSettingsViewModel(settings)
            advanceUntilIdle()
            viewModel.onApiKeyInputChanged("volcengine-key")

            viewModel.selectProvider(OnlineAsrProvider.GROQ_WHISPER)
            advanceUntilIdle()

            assertEquals("", viewModel.uiState.value.apiKeyInput)
            assertEquals(listOf(OnlineAsrProvider.GROQ_WHISPER), settings.selectedProviders)
            assertEquals(OnlineAsrProvider.GROQ_WHISPER, viewModel.uiState.value.selectedProvider)
        }

    @Test
    fun `clear targets the selected provider only`() = runTest(mainDispatcherRule.dispatcher) {
        val settings = FakeBatchCredentialSettingsPort(
            BatchCredentialConfiguration(
                selectedProvider = OnlineAsrProvider.GROQ_WHISPER,
                configuredProviders = OnlineAsrProvider.entries.toSet(),
            ),
        )
        val viewModel = CloudTranscriptionSettingsViewModel(settings)
        advanceUntilIdle()

        viewModel.clearApiKey()
        advanceUntilIdle()

        assertEquals(listOf(OnlineAsrProvider.GROQ_WHISPER), settings.clearedProviders)
    }

    @Test
    fun `port failures surface as a generic flag and never expose the key`() =
        runTest(mainDispatcherRule.dispatcher) {
            val sensitiveMessage = "sk-live-secret rejected by keystore"
            val settings = FakeBatchCredentialSettingsPort().apply {
                saveFailure = IllegalStateException(sensitiveMessage)
            }
            val viewModel = CloudTranscriptionSettingsViewModel(settings)
            advanceUntilIdle()
            viewModel.onApiKeyInputChanged("sk-live-secret")

            viewModel.saveApiKey()
            advanceUntilIdle()

            assertTrue(viewModel.uiState.value.operationFailed)
            assertFalse(viewModel.uiState.value.operationInProgress)
            assertFalse(viewModel.uiState.value.toString().contains("sk-live-secret"))
            assertFalse(viewModel.uiState.value.toString().contains(sensitiveMessage))
        }
}

private class FakeBatchCredentialSettingsPort(
    initial: BatchCredentialConfiguration = BatchCredentialConfiguration(),
) : BatchCredentialSettingsPort {
    private val mutableConfiguration = MutableStateFlow(initial)

    override val configuration: Flow<BatchCredentialConfiguration> = mutableConfiguration

    val savedCredentials = mutableListOf<Pair<OnlineAsrProvider, OnlineBatchCredential>>()
    val clearedProviders = mutableListOf<OnlineAsrProvider>()
    val selectedProviders = mutableListOf<OnlineAsrProvider>()
    var saveFailure: RuntimeException? = null

    override suspend fun selectProvider(provider: OnlineAsrProvider) {
        selectedProviders += provider
        mutableConfiguration.value = mutableConfiguration.value.copy(selectedProvider = provider)
    }

    override suspend fun save(
        provider: OnlineAsrProvider,
        credential: OnlineBatchCredential,
    ) {
        saveFailure?.let { throw it }
        savedCredentials += provider to credential
        mutableConfiguration.value = mutableConfiguration.value.copy(
            configuredProviders = mutableConfiguration.value.configuredProviders + provider,
        )
    }

    override suspend fun clear(provider: OnlineAsrProvider) {
        clearedProviders += provider
        mutableConfiguration.value = mutableConfiguration.value.copy(
            configuredProviders = mutableConfiguration.value.configuredProviders - provider,
        )
    }
}
