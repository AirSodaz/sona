package com.sona.android.adapters.android.settings

import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.emptyPreferences
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.GeneralSettings
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AndroidGeneralSettingsRepositoryTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `general settings default values and mutations persist cleanly`() = runTest {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        val dataStore = PreferenceDataStoreFactory.create(
            corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
            scope = scope,
            produceFile = { File(temporaryFolder.newFolder("ds"), "general.preferences_pb") },
        )
        try {
            val repo = AndroidGeneralSettingsRepository(dataStore, AppLogLevel.INFO)
            assertEquals(GeneralSettings(), repo.load())

            repo.setAutoCheckUpdates(false)
            repo.setWifiOnlyUpdates(true)
            repo.setIgnoredUpdateVersionCode(42)
            repo.setDownloadMirror(DownloadMirror.GH_PROXY)
            repo.setLogLevel(AppLogLevel.DEBUG)

            assertEquals(
                GeneralSettings(
                    autoCheckUpdates = false,
                    wifiOnlyUpdates = true,
                    ignoredUpdateVersionCode = 42,
                    downloadMirror = DownloadMirror.GH_PROXY,
                    logLevel = AppLogLevel.DEBUG,
                ),
                repo.settings.first(),
            )

            repo.setIgnoredUpdateVersionCode(null)
            assertNull(repo.load().ignoredUpdateVersionCode)
        } finally {
            scope.cancel()
        }
    }
}
