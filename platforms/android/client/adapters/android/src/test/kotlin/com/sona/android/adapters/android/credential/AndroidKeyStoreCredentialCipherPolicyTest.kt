package com.sona.android.adapters.android.credential

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidKeyStoreCredentialCipherPolicyTest {
    @Test
    fun `production keystore policy is fixed and api23 compatible`() {
        val policy = AndroidKeyStoreCredentialPolicy.production

        assertEquals("sona.streaming_credential.aes_gcm.v1", policy.alias)
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
        assertArrayEquals(
            "sona/android/streaming-credential/v1".encodeToByteArray(),
            policy.aad,
        )
    }

    @Test
    fun `cipher encryption API accepts plaintext only so provider owns iv creation`() {
        val encrypt = CredentialCipher::class.java.declaredMethods.single { it.name == "encrypt" }

        assertEquals(listOf(ByteArray::class.java), encrypt.parameterTypes.toList())
    }
}
