package com.sona.android.application.sync

interface SyncSecretStorePort {
    suspend fun get(logicalKey: String): ByteArray?

    suspend fun set(logicalKey: String, value: ByteArray)

    suspend fun delete(logicalKey: String)
}
