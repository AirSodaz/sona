package com.sona.android.adapters.android.sync

import android.content.Context
import com.sona.android.application.sync.SyncSecretStorePort
import java.io.File
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

class AndroidSyncSecretStore internal constructor(
    private val records: SyncSecretRecordStore,
    private val cipher: SyncSecretCipher,
    private val closeRecords: (() -> Unit)? = null,
    private val closeRecordsAndAwait: (suspend () -> Unit)? = null,
) : SyncSecretStorePort, AutoCloseable {
    private val operations = Mutex()

    override suspend fun get(logicalKey: String): ByteArray? = operations.withLock {
        val envelope = records.get(logicalKey) ?: return@withLock null
        try {
            cipher.decrypt(logicalKey, envelope)
        } catch (error: SyncSecretCipherException) {
            when (error.kind) {
                SyncSecretCipherFailureKind.PERMANENT_RECORD -> {
                    records.delete(logicalKey)
                    null
                }
                SyncSecretCipherFailureKind.PERMANENT_KEY -> {
                    recoverFromKeyInvalidation()
                    null
                }
                SyncSecretCipherFailureKind.TEMPORARY -> throw error
            }
        }
    }

    override suspend fun set(logicalKey: String, value: ByteArray) = operations.withLock {
        val envelope = try {
            encryptCopy(logicalKey, value)
        } catch (error: SyncSecretCipherException) {
            if (error.kind != SyncSecretCipherFailureKind.PERMANENT_KEY) {
                throw error
            }
            recoverFromKeyInvalidation()
            encryptCopy(logicalKey, value)
        }
        records.set(logicalKey, envelope)
    }

    private fun encryptCopy(logicalKey: String, value: ByteArray): SyncSecretEnvelope {
        val plaintext = value.copyOf()
        return try {
            cipher.encrypt(logicalKey, plaintext)
        } finally {
            plaintext.fill(0)
        }
    }

    private suspend fun recoverFromKeyInvalidation() {
        cipher.resetAfterKeyInvalidation()
        records.clear()
    }

    override suspend fun delete(logicalKey: String) = operations.withLock {
        records.delete(logicalKey)
    }

    override fun close() {
        closeRecords?.invoke()
    }

    suspend fun closeAndAwait() {
        closeRecordsAndAwait?.invoke()
    }

    companion object {
        @JvmStatic
        fun create(context: Context): AndroidSyncSecretStore = open(SyncSecretDataStore.create(context))

        @JvmStatic
        fun createForTesting(context: Context, fileName: String): AndroidSyncSecretStore =
            open(SyncSecretDataStore.createForTesting(context, fileName))

        @JvmStatic
        fun defaultFile(context: Context): File = SyncSecretDataStore.defaultFile(context)

        private fun open(records: SyncSecretDataStore): AndroidSyncSecretStore = AndroidSyncSecretStore(
            records = records,
            cipher = AndroidKeyStoreSyncSecretCipher(),
            closeRecords = records::close,
            closeRecordsAndAwait = records::closeAndAwait,
        )
    }
}
