package com.sona.android.app.feature.settings

import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.StreamingCredential
import com.sona.android.application.recording.StreamingCredentialSettingsPort
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TestWatcher
import org.junit.runner.Description

@OptIn(ExperimentalCoroutinesApi::class)
class CredentialSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `collects the initial credential status from the settings port`() =
        runTest(mainDispatcherRule.dispatcher) {
            val settings = FakeCredentialSettingsPort(CredentialStatus.CONFIGURED)

            val viewModel = CredentialSettingsViewModel(settings)
            advanceUntilIdle()

            assertEquals(CredentialStatus.CONFIGURED, viewModel.uiState.value.status)
        }

    @Test
    fun `save sends the nonblank credential input to the settings port`() =
        runTest(mainDispatcherRule.dispatcher) {
            val settings = FakeCredentialSettingsPort()
            val viewModel = CredentialSettingsViewModel(settings)
            viewModel.onCredentialInputChanged("streaming-api-key")

            viewModel.saveCredential()
            advanceUntilIdle()

            assertEquals(
                listOf(StreamingCredential("streaming-api-key")),
                settings.savedCredentials,
            )
            assertFalse(viewModel.uiState.value.operationFailed)
        }

    @Test
    fun `successful save clears the credential input`() =
        runTest(mainDispatcherRule.dispatcher) {
            val viewModel = CredentialSettingsViewModel(FakeCredentialSettingsPort())
            viewModel.onCredentialInputChanged("streaming-api-key")

            viewModel.saveCredential()
            advanceUntilIdle()

            assertEquals("", viewModel.uiState.value.credentialInput)
        }

    @Test
    fun `credential state string representation redacts plaintext input`() {
        val state = CredentialSettingsUiState(credentialInput = "streaming-api-key")

        assertFalse(state.toString().contains("streaming-api-key"))
        assertTrue(state.toString().contains("<redacted>"))
    }

    @Test
    fun `save exposes progress and ignores a duplicate command`() =
        runTest(mainDispatcherRule.dispatcher) {
            val gate = CompletableDeferred<Unit>()
            val settings = FakeCredentialSettingsPort().apply { saveGate = gate }
            val viewModel = CredentialSettingsViewModel(settings)
            viewModel.onCredentialInputChanged("streaming-api-key")

            viewModel.saveCredential()
            runCurrent()
            viewModel.saveCredential()

            assertTrue(viewModel.uiState.value.operationInProgress)
            gate.complete(Unit)
            advanceUntilIdle()
            assertEquals(1, settings.savedCredentials.size)
            assertFalse(viewModel.uiState.value.operationInProgress)
        }

    @Test
    fun `clear removes the credential through the settings port`() =
        runTest(mainDispatcherRule.dispatcher) {
            val settings = FakeCredentialSettingsPort(CredentialStatus.CONFIGURED)
            val viewModel = CredentialSettingsViewModel(settings)
            advanceUntilIdle()

            viewModel.clearCredential()
            advanceUntilIdle()

            assertEquals(1, settings.clearCalls)
            assertEquals(CredentialStatus.NOT_CONFIGURED, viewModel.uiState.value.status)
            assertFalse(viewModel.uiState.value.operationFailed)
        }

    @Test
    fun `successful clear removes transient credential input`() =
        runTest(mainDispatcherRule.dispatcher) {
            val viewModel = CredentialSettingsViewModel(
                FakeCredentialSettingsPort(CredentialStatus.CONFIGURED),
            )
            viewModel.onCredentialInputChanged("replacement-api-key")

            viewModel.clearCredential()
            advanceUntilIdle()

            assertEquals("", viewModel.uiState.value.credentialInput)
        }

    @Test
    fun `save failure is generic and keeps the credential input for retry`() =
        runTest(mainDispatcherRule.dispatcher) {
            val sensitiveFailureDetail = "keystore failed for streaming-api-key"
            val settings = FakeCredentialSettingsPort().apply {
                saveFailure = IllegalStateException(sensitiveFailureDetail)
            }
            val viewModel = CredentialSettingsViewModel(settings)
            viewModel.onCredentialInputChanged("streaming-api-key")

            viewModel.saveCredential()
            advanceUntilIdle()

            assertTrue(viewModel.uiState.value.operationFailed)
            assertEquals("streaming-api-key", viewModel.uiState.value.credentialInput)
            assertFalse(viewModel.uiState.value.toString().contains(sensitiveFailureDetail))
        }

    @Test
    fun `clear failure is generic and preserves the configured status`() =
        runTest(mainDispatcherRule.dispatcher) {
            val sensitiveFailureDetail = "credential record 42 could not be deleted"
            val settings = FakeCredentialSettingsPort(CredentialStatus.CONFIGURED).apply {
                clearFailure = IllegalStateException(sensitiveFailureDetail)
            }
            val viewModel = CredentialSettingsViewModel(settings)
            advanceUntilIdle()

            viewModel.clearCredential()
            advanceUntilIdle()

            assertTrue(viewModel.uiState.value.operationFailed)
            assertEquals(CredentialStatus.CONFIGURED, viewModel.uiState.value.status)
            assertFalse(viewModel.uiState.value.toString().contains(sensitiveFailureDetail))
        }

    @Test
    fun `constructor accepts only the credential port and no saved state persistence`() {
        val constructor = CredentialSettingsViewModel::class.java.constructors.single()

        assertEquals(
            listOf(StreamingCredentialSettingsPort::class.java),
            constructor.parameterTypes.toList(),
        )
    }
}

private class FakeCredentialSettingsPort(
    initialStatus: CredentialStatus = CredentialStatus.NOT_CONFIGURED,
) : StreamingCredentialSettingsPort {
    private val mutableStatus = MutableStateFlow(initialStatus)

    override val status: Flow<CredentialStatus> = mutableStatus

    val savedCredentials = mutableListOf<StreamingCredential>()
    var clearCalls: Int = 0
        private set
    var saveFailure: RuntimeException? = null
    var clearFailure: RuntimeException? = null
    var saveGate: CompletableDeferred<Unit>? = null

    override suspend fun save(credential: StreamingCredential) {
        saveGate?.await()
        saveFailure?.let { throw it }
        savedCredentials += credential
        mutableStatus.value = CredentialStatus.CONFIGURED
    }

    override suspend fun clear() {
        clearFailure?.let { throw it }
        clearCalls += 1
        mutableStatus.value = CredentialStatus.NOT_CONFIGURED
    }
}

@OptIn(ExperimentalCoroutinesApi::class)
class MainDispatcherRule(
    val dispatcher: TestDispatcher = StandardTestDispatcher(),
) : TestWatcher() {
    override fun starting(description: Description) {
        Dispatchers.setMain(dispatcher)
    }

    override fun finished(description: Description) {
        Dispatchers.resetMain()
    }
}
