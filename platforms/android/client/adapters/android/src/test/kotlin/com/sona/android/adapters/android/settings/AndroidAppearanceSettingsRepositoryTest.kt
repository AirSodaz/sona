package com.sona.android.adapters.android.settings

import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.emptyPreferences
import com.sona.android.application.settings.AppearanceSettings
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AndroidAppearanceSettingsRepositoryTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `settings default to dynamic color disabled`() = runTest {
        withRepository { repository ->
            assertEquals(AppearanceSettings(), repository.settings.first())
        }
    }

    @Test
    fun `dynamic color updates are persisted and published`() = runTest {
        withRepository { repository ->
            repository.setDynamicColorEnabled(true)

            assertEquals(AppearanceSettings(dynamicColorEnabled = true), repository.settings.first())
        }
    }

    private suspend fun withRepository(
        block: suspend (AndroidAppearanceSettingsRepository) -> Unit,
    ) {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        val file = File(temporaryFolder.root, AndroidAppearanceSettingsRepository.DATASTORE_FILE_NAME)
        val dataStore = PreferenceDataStoreFactory.create(
            corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
            scope = scope,
            produceFile = { file },
        )
        try {
            block(AndroidAppearanceSettingsRepository(dataStore))
        } finally {
            scope.cancel()
        }
    }
}
