package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.AppearanceSettingsPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class AppearanceSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `initial settings are loaded from the port`() = runTest {
        val settings = FakeAppearanceSettings(
            initial = AppearanceSettings(dynamicColorEnabled = true),
        )
        val viewModel = AppearanceSettingsViewModel(settings)

        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(
            AppearanceSettingsUiState(dynamicColorEnabled = true, isLoaded = true),
            viewModel.state.value,
        )
    }

    @Test
    fun `dynamic color changes are optimistic and persisted`() = runTest {
        val settings = FakeAppearanceSettings()
        val viewModel = AppearanceSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.setDynamicColorEnabled(true)
        assertEquals(true, viewModel.state.value.dynamicColorEnabled)
        assertEquals(true, viewModel.state.value.operationInProgress)

        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(true, settings.savedValue)
        assertEquals(
            AppearanceSettingsUiState(dynamicColorEnabled = true, isLoaded = true),
            viewModel.state.value,
        )
    }

    @Test
    fun `persistence failure rolls back the optimistic value`() = runTest {
        val settings = FakeAppearanceSettings().apply {
            failure = IllegalStateException("storage detail")
        }
        val viewModel = AppearanceSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.setDynamicColorEnabled(true)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(
            AppearanceSettingsUiState(isLoaded = true, hasError = true),
            viewModel.state.value,
        )
    }

    @Test
    fun `cancellation rolls back without publishing an error`() = runTest {
        val settings = FakeAppearanceSettings().apply {
            failure = CancellationException("cancelled")
        }
        val viewModel = AppearanceSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.setDynamicColorEnabled(true)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(
            AppearanceSettingsUiState(isLoaded = true),
            viewModel.state.value,
        )
    }

    private class FakeAppearanceSettings(
        initial: AppearanceSettings = AppearanceSettings(),
    ) : AppearanceSettingsPort {
        private val mutableSettings = MutableStateFlow(initial)
        override val settings: Flow<AppearanceSettings> = mutableSettings
        var savedValue: Boolean? = null
        var failure: Throwable? = null

        override suspend fun setDynamicColorEnabled(enabled: Boolean) {
            failure?.let { throw it }
            savedValue = enabled
            mutableSettings.value = AppearanceSettings(dynamicColorEnabled = enabled)
        }
    }
}
