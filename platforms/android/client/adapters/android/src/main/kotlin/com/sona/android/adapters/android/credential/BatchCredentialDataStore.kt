package com.sona.android.adapters.android.credential

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

private const val DATASTORE_UNAVAILABLE_MESSAGE = "Batch credential DataStore is unavailable."

internal class BatchCredentialDataStore private constructor(
    storageFile: File,
    private val activePath: String,
    private val lifecycleJob: Job,
) : BatchCredentialStore, AutoCloseable {
    private val dataStore: DataStore<Preferences>

    init {
        lifecycleJob.invokeOnCompletion { release(activePath) }
        dataStore = try {
            PreferenceDataStoreFactory.create(
                corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
                scope = CoroutineScope(lifecycleJob + Dispatchers.IO),
                produceFile = { storageFile },
            )
        } catch (_: Exception) {
            lifecycleJob.cancel()
            throw IllegalStateException(DATASTORE_UNAVAILABLE_MESSAGE)
        }
    }

    override val records: Flow<BatchCredentialRecords> = dataStore.data.map(::toRecords)

    override suspend fun read(): BatchCredentialRecords = records.first()

    override suspend fun writeSlot(storageId: String, record: CredentialRecord) {
        val keys = slotKeys(storageId)
        dataStore.edit { preferences ->
            preferences.remove(keys.formatVersion)
            preferences.remove(keys.iv)
            preferences.remove(keys.ciphertext)
            record.formatVersion?.let { preferences[keys.formatVersion] = it }
            record.ivBase64?.let { preferences[keys.iv] = it }
            record.ciphertextBase64?.let { preferences[keys.ciphertext] = it }
        }
    }

    override suspend fun clearSlot(storageId: String) {
        val keys = slotKeys(storageId)
        dataStore.edit { preferences ->
            preferences.remove(keys.formatVersion)
            preferences.remove(keys.iv)
            preferences.remove(keys.ciphertext)
        }
    }

    override suspend fun writeSelectedProvider(storageId: String) {
        dataStore.edit { preferences -> preferences[SELECTED_PROVIDER_KEY] = storageId }
    }

    override fun close() {
        lifecycleJob.cancel()
    }

    suspend fun closeAndAwait() {
        lifecycleJob.cancelAndJoin()
    }

    companion object {
        const val DEFAULT_FILE_NAME = "batch_credentials.preferences_pb"
        private const val DATASTORE_ACTIVE_MESSAGE = "Batch credential DataStore is already active."
        private const val SLOT_PREFIX = "credential"
        private val SELECTED_PROVIDER_KEY = stringPreferencesKey("selected_provider")
        private val activePaths = mutableSetOf<String>()

        fun create(context: Context): BatchCredentialDataStore = open(defaultFile(context))

        fun createForTesting(context: Context, fileName: String): BatchCredentialDataStore =
            open(resolveCredentialStorageFile(context.noBackupFilesDir, fileName))

        fun defaultFile(context: Context): File =
            resolveCredentialStorageFile(context.noBackupFilesDir, DEFAULT_FILE_NAME)

        private fun open(file: File): BatchCredentialDataStore {
            val canonicalFile = try {
                file.canonicalFile
            } catch (_: Exception) {
                throw IllegalStateException(DATASTORE_UNAVAILABLE_MESSAGE)
            }
            val path = canonicalFile.absolutePath
            synchronized(activePaths) {
                if (!activePaths.add(path)) {
                    throw IllegalStateException(DATASTORE_ACTIVE_MESSAGE)
                }
            }
            return BatchCredentialDataStore(
                storageFile = canonicalFile,
                activePath = path,
                lifecycleJob = SupervisorJob(),
            )
        }

        private fun release(path: String) {
            synchronized(activePaths) {
                activePaths.remove(path)
            }
        }

        private fun slotKeys(storageId: String): SlotKeys {
            require(storageId.isNotBlank()) { "Batch credential provider storage ID is blank." }
            val prefix = "$SLOT_PREFIX.$storageId"
            return SlotKeys(
                formatVersion = intPreferencesKey("$prefix.format_version"),
                iv = stringPreferencesKey("$prefix.iv_b64"),
                ciphertext = stringPreferencesKey("$prefix.ciphertext_b64"),
            )
        }

        private fun toRecords(preferences: Preferences): BatchCredentialRecords {
            val slots = buildMap {
                preferences.asMap().keys
                    .map(Preferences.Key<*>::name)
                    .mapNotNull(::slotStorageId)
                    .distinct()
                    .forEach { storageId ->
                        val keys = slotKeys(storageId)
                        put(
                            storageId,
                            CredentialRecord(
                                formatVersion = preferences[keys.formatVersion],
                                ivBase64 = preferences[keys.iv],
                                ciphertextBase64 = preferences[keys.ciphertext],
                            ),
                        )
                    }
            }
            return BatchCredentialRecords(
                selectedProviderStorageId = preferences[SELECTED_PROVIDER_KEY],
                slots = slots.filterValues { it != CredentialRecord() },
            )
        }

        private fun slotStorageId(preferenceName: String): String? {
            if (!preferenceName.startsWith("$SLOT_PREFIX.")) {
                return null
            }
            val withoutPrefix = preferenceName.removePrefix("$SLOT_PREFIX.")
            val separator = withoutPrefix.lastIndexOf('.')
            if (separator <= 0) {
                return null
            }
            return withoutPrefix.substring(0, separator).takeIf(String::isNotBlank)
        }
    }
}

private data class SlotKeys(
    val formatVersion: Preferences.Key<Int>,
    val iv: Preferences.Key<String>,
    val ciphertext: Preferences.Key<String>,
)
