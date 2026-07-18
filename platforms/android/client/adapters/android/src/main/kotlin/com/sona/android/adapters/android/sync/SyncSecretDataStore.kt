package com.sona.android.adapters.android.sync

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
import kotlin.io.encoding.Base64
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.flow.first

private const val DATASTORE_UNAVAILABLE_MESSAGE = "Sync secret DataStore is unavailable."
private const val INVALID_FILE_NAME_MESSAGE = "Sync secret DataStore test file name is invalid."

internal class SyncSecretDataStore private constructor(
    storageFile: File,
    private val activePath: String,
    private val lifecycleJob: Job,
) : SyncSecretRecordStore, AutoCloseable {
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

    override suspend fun get(logicalKey: String): SyncSecretEnvelope? {
        val keys = recordKeys(logicalKey)
        val preferences = dataStore.data.first()
        val version = preferences[keys.version]
        val ivBase64 = preferences[keys.iv]
        val ciphertextBase64 = preferences[keys.ciphertext]
        if (version == null && ivBase64 == null && ciphertextBase64 == null) {
            return null
        }
        if (version != FORMAT_VERSION || ivBase64 == null || ciphertextBase64 == null) {
            delete(logicalKey)
            return null
        }
        val iv = decodeStrict(ivBase64)
        val ciphertext = decodeStrict(ciphertextBase64)
        if (
            iv == null ||
            ciphertext == null ||
            iv.size != SyncSecretEnvelope.IV_SIZE_BYTES ||
            ciphertext.size < SyncSecretEnvelope.GCM_TAG_SIZE_BYTES
        ) {
            iv?.fill(0)
            ciphertext?.fill(0)
            delete(logicalKey)
            return null
        }
        return SyncSecretEnvelope.fromTemporaryBuffers(iv, ciphertext)
    }

    override suspend fun set(logicalKey: String, envelope: SyncSecretEnvelope) {
        val keys = recordKeys(logicalKey)
        val iv = envelope.iv
        val ciphertext = envelope.ciphertext
        try {
            dataStore.edit { preferences ->
                preferences[keys.version] = FORMAT_VERSION
                preferences[keys.iv] = Base64.Default.encode(iv)
                preferences[keys.ciphertext] = Base64.Default.encode(ciphertext)
            }
        } finally {
            iv.fill(0)
            ciphertext.fill(0)
        }
    }

    override suspend fun delete(logicalKey: String) {
        val keys = recordKeys(logicalKey)
        dataStore.edit { preferences ->
            preferences.remove(keys.version)
            preferences.remove(keys.iv)
            preferences.remove(keys.ciphertext)
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
        const val DEFAULT_FILE_NAME = "sync_secrets.preferences_pb"
        private const val FORMAT_VERSION = 1
        private const val DATASTORE_ACTIVE_MESSAGE = "Sync secret DataStore is already active."
        private val activePaths = mutableSetOf<String>()

        fun create(context: Context): SyncSecretDataStore = open(defaultFile(context))

        fun createForTesting(context: Context, fileName: String): SyncSecretDataStore =
            open(resolveStorageFile(context.noBackupFilesDir, fileName))

        fun defaultFile(context: Context): File =
            resolveStorageFile(context.noBackupFilesDir, DEFAULT_FILE_NAME)

        private fun open(file: File): SyncSecretDataStore {
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
            return SyncSecretDataStore(
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

        private fun recordKeys(logicalKey: String): RecordKeys {
            val logicalKeyBytes = logicalKey.encodeToByteArray()
            val encodedKey = try {
                Base64.Default.encode(logicalKeyBytes)
            } finally {
                logicalKeyBytes.fill(0)
            }
            val prefix = "secret.$encodedKey"
            return RecordKeys(
                version = intPreferencesKey("$prefix.version"),
                iv = stringPreferencesKey("$prefix.iv_b64"),
                ciphertext = stringPreferencesKey("$prefix.ciphertext_b64"),
            )
        }

        private fun decodeStrict(encoded: String): ByteArray? = try {
            Base64.Default.decode(encoded)
        } catch (_: IllegalArgumentException) {
            null
        }

        private fun resolveStorageFile(directory: File, fileName: String): File {
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
    }
}

private data class RecordKeys(
    val version: Preferences.Key<Int>,
    val iv: Preferences.Key<String>,
    val ciphertext: Preferences.Key<String>,
)
