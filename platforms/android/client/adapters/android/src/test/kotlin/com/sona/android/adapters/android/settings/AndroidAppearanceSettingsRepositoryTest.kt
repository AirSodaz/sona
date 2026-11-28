package com.sona.android.adapters.android.settings

import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AndroidAppearanceSettingsRepositoryTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `settings default to follow system and terracotta preset`() = runTest {
        withRepository { repository, _ ->
            assertEquals(AppearanceSettings(), repository.settings.first())
        }
    }

    @Test
    fun `all appearance settings updates are persisted and published`() = runTest {
        withRepository { repository, dataStore ->
            repository.setThemeMode(ThemeMode.DARK)
            repository.setPureBlackDarkEnabled(true)
            repository.selectPalettePreset(ColorPalettePreset.OCEAN_SLATE)
            repository.setPaletteStyle(PaletteStyle.EXPRESSIVE)
            repository.setContrastLevel(ContrastLevel.HIGH)

            assertEquals(
                AppearanceSettings(
                    themeMode = ThemeMode.DARK,
                    pureBlackDarkEnabled = true,
                    colorSource = ColorSource.PRESET,
                    palettePreset = ColorPalettePreset.OCEAN_SLATE,
                    paletteStyle = PaletteStyle.EXPRESSIVE,
                    contrastLevel = ContrastLevel.HIGH,
                ),
                repository.settings.first(),
            )

            repository.setCustomSeedColor(0x004B8378)
            assertEquals(
                AppearanceSettings(
                    themeMode = ThemeMode.DARK,
                    pureBlackDarkEnabled = true,
                    colorSource = ColorSource.CUSTOM,
                    palettePreset = ColorPalettePreset.OCEAN_SLATE,
                    customSeedColorArgb = 0xFF4B8378.toInt(),
                    paletteStyle = PaletteStyle.EXPRESSIVE,
                    contrastLevel = ContrastLevel.HIGH,
                ),
                repository.settings.first(),
            )

            repository.selectSystemDynamicColor()
            assertEquals(ColorSource.SYSTEM_DYNAMIC, repository.settings.first().colorSource)
            assertEquals(true, repository.settings.first().dynamicColorEnabled)

            // Switching back to a preset clears legacy dynamic_color_enabled
            repository.selectPalettePreset(ColorPalettePreset.TERRACOTTA)
            assertEquals(ColorSource.PRESET, repository.settings.first().colorSource)
            assertFalse(dataStore.data.first()[booleanPreferencesKey("dynamic_color_enabled")] ?: true)
        }
    }

    @Test
    fun `legacy dynamic color preference migrates to system dynamic or preset and respects explicit color source`() = runTest {
        withRepository { repository, dataStore ->
            dataStore.edit { preferences ->
                preferences[booleanPreferencesKey("dynamic_color_enabled")] = true
            }
            assertEquals(
                AppearanceSettings(colorSource = ColorSource.SYSTEM_DYNAMIC),
                repository.settings.first(),
            )

            dataStore.edit { preferences ->
                preferences[stringPreferencesKey("color_source")] = "custom"
            }
            assertEquals(
                AppearanceSettings(colorSource = ColorSource.CUSTOM),
                repository.settings.first(),
            )
        }
    }

    @Test
    fun `legacy dynamic color false and translucent custom seed normalize cleanly`() = runTest {
        withRepository { repository, dataStore ->
            dataStore.edit { preferences ->
                preferences[booleanPreferencesKey("dynamic_color_enabled")] = false
                preferences[intPreferencesKey("custom_seed_color_argb")] = 0x002B638B
            }

            assertEquals(
                AppearanceSettings(
                    colorSource = ColorSource.PRESET,
                    customSeedColorArgb = 0xFF2B638B.toInt(),
                ),
                repository.settings.first(),
            )
        }
    }

    @Test
    fun `unknown wire values safely fall back to defaults`() = runTest {
        withRepository { repository, dataStore ->
            dataStore.edit { preferences ->
                preferences[stringPreferencesKey("theme_mode")] = "invalid_mode"
                preferences[stringPreferencesKey("color_source")] = "invalid_source"
                preferences[stringPreferencesKey("palette_preset")] = "invalid_preset"
                preferences[stringPreferencesKey("palette_style")] = "invalid_style"
                preferences[stringPreferencesKey("contrast_level")] = "invalid_contrast"
            }

            assertEquals(AppearanceSettings(), repository.settings.first())
        }
    }

    private suspend fun withRepository(
        block: suspend (
            AndroidAppearanceSettingsRepository,
            androidx.datastore.core.DataStore<androidx.datastore.preferences.core.Preferences>,
        ) -> Unit,
    ) {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        val file = File(temporaryFolder.root, AndroidAppearanceSettingsRepository.DATASTORE_FILE_NAME)
        val dataStore = PreferenceDataStoreFactory.create(
            corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
            scope = scope,
            produceFile = { file },
        )
        try {
            block(AndroidAppearanceSettingsRepository(dataStore), dataStore)
        } finally {
            scope.cancel()
        }
    }
}
