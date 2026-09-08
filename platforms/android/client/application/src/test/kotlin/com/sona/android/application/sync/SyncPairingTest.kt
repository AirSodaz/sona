package com.sona.android.application.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class SyncPairingTest {
    @Test
    fun `encodes and decodes valid pairing token`() {
        val token = encodeSyncPairingToken(
            serverUrl = "https://dav.jianguoyun.com/dav/",
            remoteRoot = "Sona",
            username = "user@example.com",
            vaultId = "vault-42",
            providerPassword = "app-password-123",
        )

        assert(token.startsWith("sonasync://v1?data="))

        val payload = decodeSyncPairingToken(token)
        assertNotNull(payload)
        assertEquals(1, payload?.v)
        assertEquals("https://dav.jianguoyun.com/dav/", payload?.serverUrl)
        assertEquals("Sona", payload?.remoteRoot)
        assertEquals("user@example.com", payload?.username)
        assertEquals("vault-42", payload?.vaultId)
        assertEquals("app-password-123", payload?.providerPassword)
    }

    @Test
    fun `encodes and decodes token without provider password`() {
        val token = encodeSyncPairingToken(
            serverUrl = "https://cloud.example.com/remote.php/dav/files/user/",
            remoteRoot = "SonaVault",
            username = "user",
            vaultId = "vault-99",
        )

        val payload = decodeSyncPairingToken(token)
        assertNotNull(payload)
        assertEquals(1, payload?.v)
        assertEquals("https://cloud.example.com/remote.php/dav/files/user/", payload?.serverUrl)
        assertEquals("SonaVault", payload?.remoteRoot)
        assertEquals("user", payload?.username)
        assertEquals("vault-99", payload?.vaultId)
        assertNull(payload?.providerPassword)
    }

    @Test
    fun `decodes raw base64 string directly`() {
        val json = """{"v":1,"serverUrl":"https://example.com","remoteRoot":"Root","username":"bob","vaultId":"v1"}"""
        val rawBase64 = java.util.Base64.getEncoder().encodeToString(json.toByteArray())

        val payload = decodeSyncPairingToken(rawBase64)
        assertNotNull(payload)
        assertEquals("https://example.com", payload?.serverUrl)
        assertEquals("Root", payload?.remoteRoot)
        assertEquals("bob", payload?.username)
        assertEquals("v1", payload?.vaultId)
    }

    @Test
    fun `rejects invalid or corrupted pairing tokens`() {
        assertNull(decodeSyncPairingToken("invalid-token"))
        assertNull(decodeSyncPairingToken("sonasync://v1"))
        assertNull(decodeSyncPairingToken("sonasync://v1?data="))
        assertNull(decodeSyncPairingToken("sonasync://v1?data=bm90LWpzb24=")) // "not-json"
        assertNull(decodeSyncPairingToken(""))
    }

    @Test
    fun `detects provider preset from server URL`() {
        assertEquals(WellKnownSyncProviderId.NUTSTORE, detectProviderPresetId("https://dav.jianguoyun.com/dav/"))
        assertEquals(WellKnownSyncProviderId.NEXTCLOUD, detectProviderPresetId("https://cloud.example.com/remote.php/dav/files/user/"))
        assertEquals(WellKnownSyncProviderId.INFINICLOUD, detectProviderPresetId("https://teracloud.jp/dav/"))
        assertEquals(WellKnownSyncProviderId.SYNOLOGY, detectProviderPresetId("https://nas.example.com:5006/home/"))
        assertEquals(WellKnownSyncProviderId.ALIST, detectProviderPresetId("https://alist.example.com/dav/"))
        assertEquals(WellKnownSyncProviderId.CUSTOM, detectProviderPresetId("https://webdav.myhost.com/files/"))
    }
}
