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
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.GeneralSettings
import com.sona.android.application.settings.GeneralSettingsPort
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

class AndroidGeneralSettingsRepository internal constructor(
    private val dataStore: DataStore<Preferences>,
    private val defaultLogLevel: AppLogLevel = AppLogLevel.INFO,
) : GeneralSettingsPort {
    override val settings: Flow<GeneralSettings> = dataStore.data
        .catch { error ->
            if (error is IOException) {
                emit(emptyPreferences())
            } else {
                throw error
            }
        }
        .map { preferences ->
            GeneralSettings(
                autoCheckUpdates = preferences[AUTO_CHECK_UPDATES] ?: true,
                wifiOnlyUpdates = preferences[WIFI_ONLY_UPDATES] ?: false,
                ignoredUpdateVersionCode = preferences[IGNORED_UPDATE_VERSION_CODE],
                downloadMirror = DownloadMirror.fromWireValue(preferences[DOWNLOAD_MIRROR]),
                logLevel = AppLogLevel.fromWireValue(preferences[LOG_LEVEL], defaultLogLevel),
            )
        }
        .distinctUntilChanged()

    override suspend fun load(): GeneralSettings = settings.first()

    override suspend fun setAutoCheckUpdates(enabled: Boolean) {
        dataStore.edit { it[AUTO_CHECK_UPDATES] = enabled }
    }

    override suspend fun setWifiOnlyUpdates(enabled: Boolean) {
        dataStore.edit { it[WIFI_ONLY_UPDATES] = enabled }
    }

    override suspend fun setIgnoredUpdateVersionCode(versionCode: Int?) {
        dataStore.edit { preferences ->
            if (versionCode == null) {
                preferences.remove(IGNORED_UPDATE_VERSION_CODE)
            } else {
                preferences[IGNORED_UPDATE_VERSION_CODE] = versionCode
            }
        }
    }

    override suspend fun setDownloadMirror(mirror: DownloadMirror) {
        dataStore.edit { it[DOWNLOAD_MIRROR] = mirror.wireValue }
    }

    override suspend fun setLogLevel(level: AppLogLevel) {
        dataStore.edit { it[LOG_LEVEL] = level.wireValue }
    }

    companion object {
        private const val DATASTORE_NAME = "general_settings"
        private val AUTO_CHECK_UPDATES = booleanPreferencesKey("auto_check_updates")
        private val WIFI_ONLY_UPDATES = booleanPreferencesKey("wifi_only_updates")
        private val IGNORED_UPDATE_VERSION_CODE = intPreferencesKey("ignored_update_version_code")
        private val DOWNLOAD_MIRROR = stringPreferencesKey("download_mirror")
        private val LOG_LEVEL = stringPreferencesKey("log_level")

        @JvmStatic
        fun create(context: Context, isDebugBuild: Boolean): AndroidGeneralSettingsRepository =
            AndroidGeneralSettingsRepository(
                dataStore = PreferenceDataStoreFactory.create(
                    corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    produceFile = {
                        context.applicationContext.preferencesDataStoreFile(DATASTORE_NAME)
                    },
                ),
                defaultLogLevel = if (isDebugBuild) AppLogLevel.DEBUG else AppLogLevel.INFO,
            )
    }
}
