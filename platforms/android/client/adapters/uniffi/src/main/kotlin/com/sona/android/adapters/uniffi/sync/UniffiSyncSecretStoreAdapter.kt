package com.sona.android.adapters.uniffi.sync

import com.sona.android.application.sync.SyncSecretStorePort
import kotlinx.coroutines.CancellationException
import uniffi.sona_uniffi_bind.FfiSyncSecretStore
import uniffi.sona_uniffi_bind.SonaCoreBindingException
import uniffi.sona_uniffi_bind.registerSyncSecretStoreForAppDataDir

internal class UniffiSyncSecretStoreAdapter(
    private val delegate: SyncSecretStorePort,
) : FfiSyncSecretStore {
    override suspend fun get(key: String): ByteArray? = mapFailure {
        delegate.get(key)
    }

    override suspend fun set(key: String, value: ByteArray) {
        mapFailure { delegate.set(key, value) }
    }

    override suspend fun delete(key: String) {
        mapFailure { delegate.delete(key) }
    }

    private suspend fun <T> mapFailure(operation: suspend () -> T): T = try {
        operation()
    } catch (error: CancellationException) {
        throw error
    } catch (error: Exception) {
        throw SonaCoreBindingException.Sync(
            error.message ?: "Android sync secret store failed.",
        )
    }
}

class UniffiSyncSecretStoreRegistrar internal constructor(
    private val registerBinding: (String, FfiSyncSecretStore) -> Unit,
) {
    constructor() : this(::registerSyncSecretStoreForAppDataDir)

    fun register(appDataDir: String, store: SyncSecretStorePort) {
        registerBinding(appDataDir, UniffiSyncSecretStoreAdapter(store))
    }
}
