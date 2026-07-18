package com.sona.android.adapters.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidKeyStoreSyncSecretPolicyTest {
    @Test
    fun `production policy is fixed to the sync specific nonexportable AES GCM key`() {
        val policy = AndroidKeyStoreSyncSecretPolicy.production

        assertEquals("sona.sync_secrets.aes_gcm.v1", policy.alias)
        assertEquals("AndroidKeyStore", policy.provider)
        assertEquals("AES", policy.algorithm)
        assertEquals("AES/GCM/NoPadding", policy.transformation)
        assertEquals("GCM", policy.blockMode)
        assertEquals("NoPadding", policy.encryptionPadding)
        assertEquals(256, policy.keySizeBits)
        assertEquals(128, policy.tagSizeBits)
        assertEquals(12, policy.ivSizeBytes)
        assertTrue(policy.encryptEnabled)
        assertTrue(policy.decryptEnabled)
        assertTrue(policy.randomizedEncryptionRequired)
        assertFalse(policy.exportable)
    }

    @Test
    fun `AAD binds ciphertext to its sync logical key`() {
        val policy = AndroidKeyStoreSyncSecretPolicy.production
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"

        assertArrayEquals(
            "sona/android/sync-secret/v1/webdav-password:vault-a".encodeToByteArray(),
            policy.aadFor(passwordKey),
        )
        assertNotEquals(
            policy.aadFor(passwordKey).toList(),
            policy.aadFor(vaultKey).toList(),
        )
    }
}
