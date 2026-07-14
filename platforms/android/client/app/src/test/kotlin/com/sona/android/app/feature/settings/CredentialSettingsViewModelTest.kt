package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.StreamingCredential
import com.sona.android.application.recording.StreamingCredentialSettingsPort
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class CredentialSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `save stores the credential and publishes configured state`() = runTest {
        val settings = FakeCredentialSettings()
        val viewModel = CredentialSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.save("secret-key")
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(StreamingCredential("secret-key"), settings.savedCredential)
        assertEquals(CredentialSettingsUiState(CredentialStatus.CONFIGURED), viewModel.state.value)
    }

    @Test
    fun `blank credential and persistence errors publish the same redacted state`() = runTest {
        val settings = FakeCredentialSettings()
        val viewModel = CredentialSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.save("  ")
        assertEquals(null, settings.savedCredential)
        val expected = CredentialSettingsUiState(CredentialStatus.NOT_CONFIGURED, hasError = true)
        assertEquals(expected, viewModel.state.value)

        settings.saveFailure = IllegalStateException("keystore alias detail")
        viewModel.save("secret-key")
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(expected, viewModel.state.value)
    }

    @Test
    fun `clear removes the configured credential`() = runTest {
        val settings = FakeCredentialSettings().apply {
            statuses.value = CredentialStatus.CONFIGURED
        }
        val viewModel = CredentialSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.clear()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, settings.clearCalls)
        assertEquals(CredentialStatus.NOT_CONFIGURED, viewModel.state.value.status)
    }

    private class FakeCredentialSettings : StreamingCredentialSettingsPort {
        val statuses = MutableStateFlow(CredentialStatus.NOT_CONFIGURED)
        var savedCredential: StreamingCredential? = null
        var saveFailure: Throwable? = null
        var clearCalls = 0

        override val status: Flow<CredentialStatus> = statuses

        override suspend fun save(credential: StreamingCredential) {
            saveFailure?.let { throw it }
            savedCredential = credential
            statuses.value = CredentialStatus.CONFIGURED
        }

        override suspend fun clear() {
            clearCalls += 1
            statuses.value = CredentialStatus.NOT_CONFIGURED
        }
    }
}
