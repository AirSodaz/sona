package com.sona.android.adapters.android.credential

import com.sona.android.application.recording.CredentialStatus
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CredentialEnvelopeTest {
    @Test
    fun `empty record is not configured`() {
        val record = CredentialRecord()

        assertEquals(CredentialEnvelopeState.Empty, CredentialEnvelope.inspect(record))
        assertEquals(CredentialStatus.NOT_CONFIGURED, CredentialEnvelope.projectStatus(record))
    }

    @Test
    fun `complete version one record is configured and decodes`() {
        val iv = ByteArray(12) { it.toByte() }
        val ciphertext = ByteArray(16) { (it + 20).toByte() }
        val record = CredentialEnvelope(iv, ciphertext).toRecord()

        val state = CredentialEnvelope.inspect(record)

        assertTrue(state is CredentialEnvelopeState.Supported)
        state as CredentialEnvelopeState.Supported
        assertArrayEquals(iv, state.envelope.iv)
        assertArrayEquals(ciphertext, state.envelope.ciphertext)
        assertEquals(CredentialStatus.CONFIGURED, CredentialEnvelope.projectStatus(record))
    }

    @Test
    fun `temporary encryption buffers are copied into the envelope and cleared`() {
        val expectedIv = ByteArray(12) { (it + 1).toByte() }
        val expectedCiphertext = ByteArray(16) { (it + 20).toByte() }
        val temporaryIv = expectedIv.copyOf()
        val temporaryCiphertext = expectedCiphertext.copyOf()

        val envelope = CredentialEnvelope.fromTemporaryBuffers(
            iv = temporaryIv,
            ciphertext = temporaryCiphertext,
        )

        assertArrayEquals(expectedIv, envelope.iv)
        assertArrayEquals(expectedCiphertext, envelope.ciphertext)
        assertTrue(temporaryIv.all { it == 0.toByte() })
        assertTrue(temporaryCiphertext.all { it == 0.toByte() })
    }

    @Test
    fun `partial records are malformed and not configured`() {
        val complete = CredentialEnvelope(ByteArray(12), ByteArray(16)).toRecord()
        val partialRecords = listOf(
            CredentialRecord(formatVersion = 1),
            CredentialRecord(formatVersion = 1, ivBase64 = complete.ivBase64),
            CredentialRecord(formatVersion = 1, ciphertextBase64 = complete.ciphertextBase64),
            CredentialRecord(ivBase64 = complete.ivBase64, ciphertextBase64 = complete.ciphertextBase64),
        )

        partialRecords.forEach { record ->
            assertEquals(CredentialEnvelopeState.Malformed, CredentialEnvelope.inspect(record))
            assertEquals(CredentialStatus.NOT_CONFIGURED, CredentialEnvelope.projectStatus(record))
        }
    }

    @Test
    fun `base64 decoding is strict and canonical`() {
        val complete = CredentialEnvelope(ByteArray(12) { 0xfb.toByte() }, ByteArray(16)).toRecord()
        val ivBase64 = requireNotNull(complete.ivBase64)
        val ciphertextBase64 = requireNotNull(complete.ciphertextBase64)
        val malformedRecords = listOf(
            complete.copy(ivBase64 = ivBase64 + "\n"),
            complete.copy(ivBase64 = ivBase64.replace('+', '-')),
            complete.copy(ciphertextBase64 = ciphertextBase64.dropLast(1)),
            complete.copy(ciphertextBase64 = ciphertextBase64.dropLast(2) + "!!"),
            complete.copy(ciphertextBase64 = ciphertextBase64 + "===="),
        )

        malformedRecords.forEach { record ->
            assertEquals(CredentialEnvelopeState.Malformed, CredentialEnvelope.inspect(record))
        }
    }

    @Test
    fun `version one requires twelve byte iv and at least a sixteen byte tag`() {
        listOf(0, 11, 13).forEach { ivSize ->
            val record = CredentialEnvelope.encodeUnchecked(
                formatVersion = 1,
                iv = ByteArray(ivSize),
                ciphertext = ByteArray(16),
            )
            assertEquals(CredentialEnvelopeState.Malformed, CredentialEnvelope.inspect(record))
        }
        listOf(0, 15).forEach { ciphertextSize ->
            val record = CredentialEnvelope.encodeUnchecked(
                formatVersion = 1,
                iv = ByteArray(12),
                ciphertext = ByteArray(ciphertextSize),
            )
            assertEquals(CredentialEnvelopeState.Malformed, CredentialEnvelope.inspect(record))
        }

        val tagOnly = CredentialEnvelope(ByteArray(12), ByteArray(16)).toRecord()
        assertTrue(CredentialEnvelope.inspect(tagOnly) is CredentialEnvelopeState.Supported)
    }

    @Test
    fun `unknown version is recognized without interpreting its payload`() {
        val record = CredentialRecord(formatVersion = 9)

        assertEquals(CredentialEnvelopeState.Unsupported(9), CredentialEnvelope.inspect(record))
        assertEquals(CredentialStatus.CONFIGURED, CredentialEnvelope.projectStatus(record))
    }

    @Test
    fun `non-positive versions are malformed rather than future formats`() {
        listOf(0, -1, Int.MIN_VALUE).forEach { version ->
            val record = CredentialRecord(
                formatVersion = version,
                ivBase64 = "corrupt-iv",
                ciphertextBase64 = "corrupt-ciphertext",
            )

            assertEquals(CredentialEnvelopeState.Malformed, CredentialEnvelope.inspect(record))
            assertEquals(CredentialStatus.NOT_CONFIGURED, CredentialEnvelope.projectStatus(record))
        }
    }
}
