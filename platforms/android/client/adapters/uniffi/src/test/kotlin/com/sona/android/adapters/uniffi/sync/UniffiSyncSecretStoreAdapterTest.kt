package com.sona.android.adapters.uniffi.sync

import com.sona.android.application.sync.SyncSecretStorePort
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiSyncSecretStore
import uniffi.sona_uniffi_bind.SonaCoreBindingException

class UniffiSyncSecretStoreAdapterTest {
    @Test
    fun `adapter delegates get set and delete with the logical key intact`() = runTest {
        val port = RecordingSyncSecretStore()
        val adapter = UniffiSyncSecretStoreAdapter(port)

        adapter.set("webdav-password:vault-a", byteArrayOf(1, 2, 3))
        assertArrayEquals(
            byteArrayOf(1, 2, 3),
            adapter.get("webdav-password:vault-a"),
        )
        adapter.delete("webdav-password:vault-a")

        assertNull(adapter.get("webdav-password:vault-a"))
        assertEquals(
            listOf(
                "set:webdav-password:vault-a",
                "get:webdav-password:vault-a",
                "delete:webdav-password:vault-a",
                "get:webdav-password:vault-a",
            ),
            port.calls,
        )
    }

    @Test
    fun `registrar passes a working adapter to the generated binding`() = runTest {
        var registeredAppDataDir: String? = null
        var registered: FfiSyncSecretStore? = null
        val registrar = UniffiSyncSecretStoreRegistrar { appDataDir, store ->
            registeredAppDataDir = appDataDir
            registered = store
        }
        val port = RecordingSyncSecretStore()

        registrar.register("/data/user/0/com.sona/files", port)
        checkNotNull(registered).set("vault-key:vault-a", byteArrayOf(9))

        assertEquals("/data/user/0/com.sona/files", registeredAppDataDir)
        assertArrayEquals(byteArrayOf(9), port.get("vault-key:vault-a"))
    }

    @Test
    fun `adapter maps host storage failures to the existing sync error variant`() = runTest {
        val adapter = UniffiSyncSecretStoreAdapter(FailingSyncSecretStore())

        try {
            adapter.get("vault-key:vault-a")
            fail("expected the generated Sync error variant")
        } catch (error: SonaCoreBindingException.Sync) {
            assertTrue(error.message.orEmpty().contains("secure storage unavailable"))
        }
    }
}

private class RecordingSyncSecretStore : SyncSecretStorePort {
    private val values = mutableMapOf<String, ByteArray>()
    val calls = mutableListOf<String>()

    override suspend fun get(logicalKey: String): ByteArray? {
        calls += "get:$logicalKey"
        return values[logicalKey]?.copyOf()
    }

    override suspend fun set(logicalKey: String, value: ByteArray) {
        calls += "set:$logicalKey"
        values[logicalKey] = value.copyOf()
    }

    override suspend fun delete(logicalKey: String) {
        calls += "delete:$logicalKey"
        values.remove(logicalKey)
    }
}

private class FailingSyncSecretStore : SyncSecretStorePort {
    override suspend fun get(logicalKey: String): ByteArray? =
        error("secure storage unavailable")

    override suspend fun set(logicalKey: String, value: ByteArray) =
        error("secure storage unavailable")

    override suspend fun delete(logicalKey: String) =
        error("secure storage unavailable")
}
