package com.sona.android.adapters.android.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.AppearanceSettingsPort
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

class AndroidAppearanceSettingsRepository internal constructor(
    private val dataStore: DataStore<Preferences>,
) : AppearanceSettingsPort {
    override val settings: Flow<AppearanceSettings> = dataStore.data
        .catch { error ->
            if (error is IOException) {
                emit(emptyPreferences())
            } else {
                throw error
            }
        }
        .map { preferences ->
            val storedColorSource = preferences[COLOR_SOURCE]
            val resolvedColorSource = when {
                storedColorSource != null -> ColorSource.fromWireValue(storedColorSource)
                preferences[DYNAMIC_COLOR_ENABLED] == true -> ColorSource.SYSTEM_DYNAMIC
                else -> ColorSource.PRESET
            }
            AppearanceSettings(
                themeMode = ThemeMode.fromWireValue(preferences[THEME_MODE]),
                pureBlackDarkEnabled = preferences[PURE_BLACK_DARK_ENABLED] ?: false,
                colorSource = resolvedColorSource,
                palettePreset = ColorPalettePreset.fromWireValue(preferences[PALETTE_PRESET]),
                customSeedColorArgb = (
                    preferences[CUSTOM_SEED_COLOR_ARGB]
                        ?: AppearanceSettings.DEFAULT_CUSTOM_SEED_COLOR_ARGB
                    ) or 0xFF000000.toInt(),
                paletteStyle = PaletteStyle.fromWireValue(preferences[PALETTE_STYLE]),
                contrastLevel = ContrastLevel.fromWireValue(preferences[CONTRAST_LEVEL]),
            )
        }
        .distinctUntilChanged()

    override suspend fun setThemeMode(mode: ThemeMode) {
        dataStore.edit { preferences ->
            preferences[THEME_MODE] = mode.wireValue
        }
    }

    override suspend fun setPureBlackDarkEnabled(enabled: Boolean) {
        dataStore.edit { preferences ->
            preferences[PURE_BLACK_DARK_ENABLED] = enabled
        }
    }

    override suspend fun selectSystemDynamicColor() {
        dataStore.edit { preferences ->
            preferences[COLOR_SOURCE] = ColorSource.SYSTEM_DYNAMIC.wireValue
            preferences[DYNAMIC_COLOR_ENABLED] = true
        }
    }

    override suspend fun selectPalettePreset(preset: ColorPalettePreset) {
        dataStore.edit { preferences ->
            preferences[COLOR_SOURCE] = ColorSource.PRESET.wireValue
            preferences[PALETTE_PRESET] = preset.wireValue
            preferences[DYNAMIC_COLOR_ENABLED] = false
        }
    }

    override suspend fun setCustomSeedColor(argb: Int) {
        dataStore.edit { preferences ->
            preferences[COLOR_SOURCE] = ColorSource.CUSTOM.wireValue
            preferences[CUSTOM_SEED_COLOR_ARGB] = argb or 0xFF000000.toInt()
            preferences[DYNAMIC_COLOR_ENABLED] = false
        }
    }

    override suspend fun setPaletteStyle(style: PaletteStyle) {
        dataStore.edit { preferences ->
            preferences[PALETTE_STYLE] = style.wireValue
        }
    }

    override suspend fun setContrastLevel(level: ContrastLevel) {
        dataStore.edit { preferences ->
            preferences[CONTRAST_LEVEL] = level.wireValue
        }
    }

    companion object {
        internal const val DATASTORE_FILE_NAME = "appearance_settings.preferences_pb"
        private const val DATASTORE_NAME = "appearance_settings"
        private val DYNAMIC_COLOR_ENABLED = booleanPreferencesKey("dynamic_color_enabled")
        private val THEME_MODE = stringPreferencesKey("theme_mode")
        private val PURE_BLACK_DARK_ENABLED = booleanPreferencesKey("pure_black_dark_enabled")
        private val COLOR_SOURCE = stringPreferencesKey("color_source")
        private val PALETTE_PRESET = stringPreferencesKey("palette_preset")
        private val CUSTOM_SEED_COLOR_ARGB = intPreferencesKey("custom_seed_color_argb")
        private val PALETTE_STYLE = stringPreferencesKey("palette_style")
        private val CONTRAST_LEVEL = stringPreferencesKey("contrast_level")

        @JvmStatic
        fun create(context: Context): AndroidAppearanceSettingsRepository =
            AndroidAppearanceSettingsRepository(
                PreferenceDataStoreFactory.create(
                    corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    produceFile = {
                        context.applicationContext.preferencesDataStoreFile(DATASTORE_NAME)
                    },
                ),
            )
    }
}
