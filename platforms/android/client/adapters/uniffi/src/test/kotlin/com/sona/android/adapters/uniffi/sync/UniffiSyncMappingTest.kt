package com.sona.android.adapters.uniffi.sync

import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.WebDavSyncProvider
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiSyncErrorSnapshotV1
import uniffi.sona_uniffi_bind.FfiSyncLifecycleStateV1
import uniffi.sona_uniffi_bind.FfiSyncPresetV1
import uniffi.sona_uniffi_bind.FfiSyncRunResultV1
import uniffi.sona_uniffi_bind.FfiSyncStatusSnapshotV1

class UniffiSyncMappingTest {
    @Test
    fun `WebDAV provider requires HTTPS and uses structured JSON`() {
        val ffi = WebDavSyncProvider(" HTTPS://dav.example ", " Sona ", " user ", "secret").toFfi()
        val json = Json.parseToJsonElement(ffi.configurationJson).jsonObject

        assertEquals("https://dav.example", json.getValue("serverUrl").jsonPrimitive.content.lowercase())
        assertEquals("Sona", json.getValue("remoteRoot").jsonPrimitive.content)
        assertEquals("user", json.getValue("username").jsonPrimitive.content)
        assertEquals("secret", json.getValue("password").jsonPrimitive.content)
        assertThrows(IllegalArgumentException::class.java) {
            WebDavSyncProvider("http://dav.example", "Sona", "u", "p").toFfi()
        }
    }

    @Test
    fun `maps structured sync status and retryability`() {
        val mapped = FfiSyncStatusSnapshotV1(
            FfiSyncLifecycleStateV1.ERROR,
            "webdav",
            "vault-1",
            FfiSyncPresetV1.STANDARD,
            100uL,
            2uL,
            3uL,
            200uL,
            FfiSyncErrorSnapshotV1("timeout", "Timed out", true),
        ).toApplication()

        assertEquals(SyncLifecycleState.ERROR, mapped.state)
        assertEquals(2, mapped.pendingOperationCount)
        assertTrue(mapped.lastError?.retryable == true)
    }

    @Test
    fun `maps run counts and rejects overflow`() {
        val result = FfiSyncRunResultV1(1uL, 0uL, 2uL, 3uL, 4uL, 5uL, false).toApplication()
        assertEquals(5, result.conflictCount)
        assertThrows(IllegalArgumentException::class.java) {
            FfiSyncRunResultV1(ULong.MAX_VALUE, 0uL, 0uL, 0uL, 0uL, 0uL, false).toApplication()
        }
    }
}
