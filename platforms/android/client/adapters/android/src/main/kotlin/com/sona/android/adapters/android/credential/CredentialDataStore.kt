package com.sona.android.adapters.android.credential

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
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

private const val DATASTORE_UNAVAILABLE_MESSAGE = "Credential DataStore is unavailable."
private const val INVALID_FILE_NAME_MESSAGE = "Credential DataStore test file name is invalid."

internal fun resolveCredentialStorageFile(
    directory: File,
    fileName: String,
): File {
    if (
        fileName.isBlank() ||
        fileName == "." ||
        fileName == ".." ||
        fileName != File(fileName).name
    ) {
        throw IllegalArgumentException(INVALID_FILE_NAME_MESSAGE)
    }
    val canonicalDirectory = try {
        directory.canonicalFile
    } catch (_: Exception) {
        throw IllegalStateException(DATASTORE_UNAVAILABLE_MESSAGE)
    }
    val canonicalFile = try {
        File(canonicalDirectory, fileName).canonicalFile
    } catch (_: Exception) {
        throw IllegalStateException(DATASTORE_UNAVAILABLE_MESSAGE)
    }
    if (canonicalFile.parentFile != canonicalDirectory) {
        throw IllegalArgumentException(INVALID_FILE_NAME_MESSAGE)
    }
    return canonicalFile
}

internal class CredentialDataStore private constructor(
    private val storageFile: File,
    private val activePath: String,
    private val lifecycleJob: Job,
) : CredentialStore, AutoCloseable {
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

    override val records: Flow<CredentialRecord> = dataStore.data.map(::toRecord)

    override suspend fun read(): CredentialRecord = records.first()

    override suspend fun write(record: CredentialRecord) {
        dataStore.edit { preferences ->
            preferences.clear()
            record.formatVersion?.let { preferences[FORMAT_VERSION_KEY] = it }
            record.ivBase64?.let { preferences[IV_BASE64_KEY] = it }
            record.ciphertextBase64?.let { preferences[CIPHERTEXT_BASE64_KEY] = it }
        }
    }

    override suspend fun clear() {
        dataStore.edit { preferences -> preferences.clear() }
    }

    override fun close() {
        lifecycleJob.cancel()
    }

    suspend fun closeAndAwait() {
        lifecycleJob.cancelAndJoin()
    }

    companion object {
        const val DEFAULT_FILE_NAME = "streaming_credentials.preferences_pb"
        private const val DATASTORE_ACTIVE_MESSAGE = "Credential DataStore is already active."
        private val FORMAT_VERSION_KEY = intPreferencesKey("format_version")
        private val IV_BASE64_KEY = stringPreferencesKey("iv_b64")
        private val CIPHERTEXT_BASE64_KEY = stringPreferencesKey("ciphertext_b64")
        private val activePaths = mutableSetOf<String>()

        fun create(context: Context): CredentialDataStore = open(
            resolveCredentialStorageFile(context.noBackupFilesDir, DEFAULT_FILE_NAME),
        )

        fun createForTesting(context: Context, fileName: String): CredentialDataStore =
            open(resolveCredentialStorageFile(context.noBackupFilesDir, fileName))

        fun defaultFile(context: Context): File =
            resolveCredentialStorageFile(context.noBackupFilesDir, DEFAULT_FILE_NAME)

        private fun open(file: File): CredentialDataStore {
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
            return CredentialDataStore(
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

        private fun toRecord(preferences: Preferences): CredentialRecord = CredentialRecord(
            formatVersion = preferences[FORMAT_VERSION_KEY],
            ivBase64 = preferences[IV_BASE64_KEY],
            ciphertextBase64 = preferences[CIPHERTEXT_BASE64_KEY],
        )
    }
}
