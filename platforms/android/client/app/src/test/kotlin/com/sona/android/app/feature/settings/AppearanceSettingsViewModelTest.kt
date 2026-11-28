package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.AppearanceSettingsPort
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class AppearanceSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `initial settings are loaded from the port`() = runTest {
        val settings = FakeAppearanceSettings(
            initial = AppearanceSettings(
                themeMode = ThemeMode.DARK,
                pureBlackDarkEnabled = true,
                colorSource = ColorSource.SYSTEM_DYNAMIC,
                palettePreset = ColorPalettePreset.OCEAN_SLATE,
                paletteStyle = PaletteStyle.VIBRANT,
                contrastLevel = ContrastLevel.HIGH,
            ),
        )
        val viewModel = AppearanceSettingsViewModel(settings)

        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(
            AppearanceSettingsUiState(
                themeMode = ThemeMode.DARK,
                pureBlackDarkEnabled = true,
                colorSource = ColorSource.SYSTEM_DYNAMIC,
                palettePreset = ColorPalettePreset.OCEAN_SLATE,
                paletteStyle = PaletteStyle.VIBRANT,
                contrastLevel = ContrastLevel.HIGH,
                isLoaded = true,
            ),
            viewModel.state.value,
        )
    }

    @Test
    fun `theme mode, pure black, palette, style and contrast changes are optimistic and persisted`() = runTest {
        val settings = FakeAppearanceSettings()
        val viewModel = AppearanceSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.setThemeMode(ThemeMode.LIGHT)
        assertEquals(ThemeMode.LIGHT, viewModel.state.value.themeMode)
        assertEquals(true, viewModel.state.value.operationInProgress)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(ThemeMode.LIGHT, settings.current.themeMode)
        assertEquals(false, viewModel.state.value.operationInProgress)

        viewModel.setPureBlackDarkEnabled(true)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(true, settings.current.pureBlackDarkEnabled)

        viewModel.selectPalettePreset(ColorPalettePreset.SAGE_FOREST)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(ColorSource.PRESET, settings.current.colorSource)
        assertEquals(ColorPalettePreset.SAGE_FOREST, settings.current.palettePreset)

        viewModel.setCustomSeedColor(0x00123456)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(ColorSource.CUSTOM, settings.current.colorSource)
        assertEquals(0xFF123456.toInt(), settings.current.customSeedColorArgb)

        // Alpha-equivalent duplicate seed color does not trigger another persistence write
        val writeCountBeforeNoOp = settings.writeCount
        viewModel.setCustomSeedColor(0xFF123456.toInt())
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(writeCountBeforeNoOp, settings.writeCount)

        viewModel.selectSystemDynamicColor()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(ColorSource.SYSTEM_DYNAMIC, settings.current.colorSource)

        viewModel.setPaletteStyle(PaletteStyle.EXPRESSIVE)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(PaletteStyle.EXPRESSIVE, settings.current.paletteStyle)

        viewModel.setContrastLevel(ContrastLevel.MEDIUM)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(ContrastLevel.MEDIUM, settings.current.contrastLevel)
    }

    @Test
    fun `rapid sequential updates are serialized without dropping clicks or losing emissions`() = runTest {
        val gate = CompletableDeferred<Unit>()
        val settings = FakeAppearanceSettings(writeGate = gate)
        val viewModel = AppearanceSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.setThemeMode(ThemeMode.DARK)
        viewModel.setPureBlackDarkEnabled(true)
        viewModel.selectPalettePreset(ColorPalettePreset.OCEAN_SLATE)

        assertEquals(ThemeMode.DARK, viewModel.state.value.themeMode)
        assertTrue(viewModel.state.value.pureBlackDarkEnabled)
        assertEquals(ColorPalettePreset.OCEAN_SLATE, viewModel.state.value.palettePreset)
        assertTrue(viewModel.state.value.operationInProgress)

        gate.complete(Unit)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertFalse(viewModel.state.value.operationInProgress)
        assertEquals(ThemeMode.DARK, settings.current.themeMode)
        assertTrue(settings.current.pureBlackDarkEnabled)
        assertEquals(ColorPalettePreset.OCEAN_SLATE, settings.current.palettePreset)
        assertEquals(ThemeMode.DARK, viewModel.state.value.themeMode)
        assertTrue(viewModel.state.value.pureBlackDarkEnabled)
        assertEquals(ColorPalettePreset.OCEAN_SLATE, viewModel.state.value.palettePreset)
    }

    @Test
    fun `updates before initial load are ignored`() = runTest {
        val settings = FakeAppearanceSettings()
        val viewModel = AppearanceSettingsViewModel(settings)

        viewModel.setThemeMode(ThemeMode.DARK)
        assertEquals(0, settings.writeCount)
        assertFalse(viewModel.state.value.isLoaded)
    }

    @Test
    fun `upstream flow error still marks state loaded with error flag`() = runTest {
        val brokenPort = object : AppearanceSettingsPort {
            override val settings: Flow<AppearanceSettings> = flow {
                throw IllegalStateException("corrupt datastore")
            }

            override suspend fun setThemeMode(mode: ThemeMode) = Unit
            override suspend fun setPureBlackDarkEnabled(enabled: Boolean) = Unit
            override suspend fun selectSystemDynamicColor() = Unit
            override suspend fun selectPalettePreset(preset: ColorPalettePreset) = Unit
            override suspend fun setCustomSeedColor(argb: Int) = Unit
            override suspend fun setPaletteStyle(style: PaletteStyle) = Unit
            override suspend fun setContrastLevel(level: ContrastLevel) = Unit
        }
        val viewModel = AppearanceSettingsViewModel(brokenPort)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertTrue(viewModel.state.value.isLoaded)
        assertTrue(viewModel.state.value.hasError)
    }

    @Test
    fun `persistence failure rolls back the optimistic value`() = runTest {
        val settings = FakeAppearanceSettings().apply {
            failure = IllegalStateException("storage detail")
        }
        val viewModel = AppearanceSettingsViewModel(settings)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        viewModel.setThemeMode(ThemeMode.DARK)
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

        viewModel.setPureBlackDarkEnabled(true)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(
            AppearanceSettingsUiState(isLoaded = true),
            viewModel.state.value,
        )
    }

    @Test
    fun `partial failure in rapid sequential updates preserves earlier write and retains error across delayed emission`() = runTest {
        val delayedEmission = MutableStateFlow(AppearanceSettings())
        val firstWriteGate = CompletableDeferred<Unit>()
        val sequencedPort = object : AppearanceSettingsPort {
            override val settings: Flow<AppearanceSettings> = delayedEmission

            override suspend fun setThemeMode(mode: ThemeMode) {
                firstWriteGate.await()
            }

            override suspend fun setPureBlackDarkEnabled(enabled: Boolean) {
                throw IllegalStateException("second write failed")
            }

            override suspend fun selectSystemDynamicColor() = Unit
            override suspend fun selectPalettePreset(preset: ColorPalettePreset) = Unit
            override suspend fun setCustomSeedColor(argb: Int) = Unit
            override suspend fun setPaletteStyle(style: PaletteStyle) = Unit
            override suspend fun setContrastLevel(level: ContrastLevel) = Unit
        }
        val sequencedViewModel = AppearanceSettingsViewModel(sequencedPort)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        sequencedViewModel.setThemeMode(ThemeMode.DARK)
        sequencedViewModel.setPureBlackDarkEnabled(true)
        firstWriteGate.complete(Unit)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(ThemeMode.DARK, sequencedViewModel.state.value.themeMode)
        assertFalse(sequencedViewModel.state.value.pureBlackDarkEnabled)
        assertFalse(sequencedViewModel.state.value.operationInProgress)
        assertTrue(sequencedViewModel.state.value.hasError)

        // Delayed DataStore emission for the first successful write must not clear the second write's error
        delayedEmission.value = AppearanceSettings(themeMode = ThemeMode.DARK)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(ThemeMode.DARK, sequencedViewModel.state.value.themeMode)
        assertFalse(sequencedViewModel.state.value.pureBlackDarkEnabled)
        assertTrue(sequencedViewModel.state.value.hasError)
    }

    private class FakeAppearanceSettings(
        initial: AppearanceSettings = AppearanceSettings(),
        private val writeGate: CompletableDeferred<Unit>? = null,
    ) : AppearanceSettingsPort {
        private val mutableSettings = MutableStateFlow(initial)
        override val settings: Flow<AppearanceSettings> = mutableSettings
        val current: AppearanceSettings
            get() = mutableSettings.value
        var writeCount: Int = 0
        var failure: Throwable? = null

        override suspend fun setThemeMode(mode: ThemeMode) {
            update { it.copy(themeMode = mode) }
        }

        override suspend fun setPureBlackDarkEnabled(enabled: Boolean) {
            update { it.copy(pureBlackDarkEnabled = enabled) }
        }

        override suspend fun selectSystemDynamicColor() {
            update { it.copy(colorSource = ColorSource.SYSTEM_DYNAMIC) }
        }

        override suspend fun selectPalettePreset(preset: ColorPalettePreset) {
            update { it.copy(colorSource = ColorSource.PRESET, palettePreset = preset) }
        }

        override suspend fun setCustomSeedColor(argb: Int) {
            update { it.copy(colorSource = ColorSource.CUSTOM, customSeedColorArgb = argb) }
        }

        override suspend fun setPaletteStyle(style: PaletteStyle) {
            update { it.copy(paletteStyle = style) }
        }

        override suspend fun setContrastLevel(level: ContrastLevel) {
            update { it.copy(contrastLevel = level) }
        }

        private suspend fun update(transform: (AppearanceSettings) -> AppearanceSettings) {
            writeGate?.await()
            failure?.let { throw it }
            writeCount += 1
            mutableSettings.value = transform(mutableSettings.value)
        }
    }
}
