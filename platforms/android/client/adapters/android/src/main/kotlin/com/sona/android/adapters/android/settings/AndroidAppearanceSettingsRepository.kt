package com.sona.android.adapters.android.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.preferencesDataStoreFile
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.AppearanceSettingsPort
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
            AppearanceSettings(
                dynamicColorEnabled = preferences[DYNAMIC_COLOR_ENABLED] ?: false,
            )
        }
        .distinctUntilChanged()

    override suspend fun setDynamicColorEnabled(enabled: Boolean) {
        dataStore.edit { preferences ->
            preferences[DYNAMIC_COLOR_ENABLED] = enabled
        }
    }

    companion object {
        internal const val DATASTORE_FILE_NAME = "appearance_settings.preferences_pb"
        private const val DATASTORE_NAME = "appearance_settings"
        private val DYNAMIC_COLOR_ENABLED = booleanPreferencesKey("dynamic_color_enabled")

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
