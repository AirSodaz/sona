package com.sona.android.adapters.android.credential

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertThrows
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
    fun `each batch provider policy owns a distinct alias and aad binding`() {
        val policies = listOf("volcengine-doubao", "groq-whisper", "mistral-voxtral")
            .associateWith(AndroidKeyStoreCredentialPolicy::batch)

        policies.forEach { (storageId, policy) ->
            assertEquals("sona.batch_credential.$storageId.aes_gcm.v1", policy.alias)
            assertArrayEquals(
                "sona/android/batch-credential/v1/$storageId".encodeToByteArray(),
                policy.aad,
            )
            assertEquals(AndroidKeyStoreCredentialPolicy.production.keySizeBits, policy.keySizeBits)
            assertEquals(
                AndroidKeyStoreCredentialPolicy.production.transformation,
                policy.transformation,
            )
            assertTrue(policy.randomizedEncryptionRequired)
            assertFalse(policy.exportable)
        }
        assertEquals(3, policies.values.map(AndroidKeyStoreCredentialPolicy::alias).toSet().size)
        assertNotEquals(
            AndroidKeyStoreCredentialPolicy.production.alias,
            policies.values.first().alias,
        )
    }

    @Test
    fun `batch policies reject storage ids that could escape their namespace`() {
        listOf("", " ", "Groq", "groq_whisper", "../other", "groq-", "groq whisper").forEach {
            assertThrows(IllegalArgumentException::class.java) {
                AndroidKeyStoreCredentialPolicy.batch(it)
            }
        }
    }
}
