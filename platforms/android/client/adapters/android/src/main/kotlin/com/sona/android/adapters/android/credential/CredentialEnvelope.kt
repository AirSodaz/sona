package com.sona.android.adapters.android.credential

import com.sona.android.application.recording.CredentialStatus
import kotlin.io.encoding.Base64

internal data class CredentialRecord(
    val formatVersion: Int? = null,
    val ivBase64: String? = null,
    val ciphertextBase64: String? = null,
) {
    override fun toString(): String =
        "CredentialRecord(formatVersion=<redacted>, ivBase64=<redacted>, ciphertextBase64=<redacted>)"
}

internal class CredentialEnvelope(
    iv: ByteArray,
    ciphertext: ByteArray,
) {
    private val ivBytes: ByteArray
    private val ciphertextBytes: ByteArray

    val iv: ByteArray
        get() = ivBytes.copyOf()

    val ciphertext: ByteArray
        get() = ciphertextBytes.copyOf()

    init {
        require(iv.size == IV_SIZE_BYTES) { "Credential envelope IV has an invalid size." }
        require(ciphertext.size >= GCM_TAG_SIZE_BYTES) {
            "Credential envelope ciphertext has an invalid size."
        }
        ivBytes = iv.copyOf()
        ciphertextBytes = ciphertext.copyOf()
    }

    fun toRecord(): CredentialRecord = encodeUnchecked(
        formatVersion = FORMAT_VERSION,
        iv = ivBytes,
        ciphertext = ciphertextBytes,
    )

    override fun toString(): String = "CredentialEnvelope(iv=<redacted>, ciphertext=<redacted>)"

    companion object {
        const val FORMAT_VERSION = 1
        const val IV_SIZE_BYTES = 12
        const val GCM_TAG_SIZE_BYTES = 16

        fun inspect(record: CredentialRecord): CredentialEnvelopeState {
            if (record == CredentialRecord()) {
                return CredentialEnvelopeState.Empty
            }
            val version = record.formatVersion
            if (version != null && version <= 0) {
                return CredentialEnvelopeState.Malformed
            }
            if (version != null && version != FORMAT_VERSION) {
                return CredentialEnvelopeState.Unsupported(version)
            }
            if (version == null || record.ivBase64 == null || record.ciphertextBase64 == null) {
                return CredentialEnvelopeState.Malformed
            }
            val iv = decodeStrict(record.ivBase64) ?: return CredentialEnvelopeState.Malformed
            val ciphertext = decodeStrict(record.ciphertextBase64) ?: run {
                iv.fill(0)
                return CredentialEnvelopeState.Malformed
            }
            if (iv.size != IV_SIZE_BYTES || ciphertext.size < GCM_TAG_SIZE_BYTES) {
                iv.fill(0)
                ciphertext.fill(0)
                return CredentialEnvelopeState.Malformed
            }
            return CredentialEnvelopeState.Supported(fromTemporaryBuffers(iv, ciphertext))
        }

        fun projectStatus(record: CredentialRecord): CredentialStatus = when (val state = inspect(record)) {
            CredentialEnvelopeState.Empty,
            CredentialEnvelopeState.Malformed,
            -> CredentialStatus.NOT_CONFIGURED

            is CredentialEnvelopeState.Supported -> CredentialStatus.CONFIGURED
            is CredentialEnvelopeState.Unsupported -> CredentialStatus.CONFIGURED
        }

        fun encodeUnchecked(
            formatVersion: Int,
            iv: ByteArray,
            ciphertext: ByteArray,
        ): CredentialRecord = CredentialRecord(
            formatVersion = formatVersion,
            ivBase64 = Base64.Default.encode(iv),
            ciphertextBase64 = Base64.Default.encode(ciphertext),
        )

        fun fromTemporaryBuffers(
            iv: ByteArray,
            ciphertext: ByteArray,
        ): CredentialEnvelope = try {
            CredentialEnvelope(iv, ciphertext)
        } finally {
            iv.fill(0)
            ciphertext.fill(0)
        }

        private fun decodeStrict(encoded: String): ByteArray? {
            if (encoded.isEmpty() || encoded.length % 4 != 0) {
                return null
            }
            val firstPadding = encoded.indexOf('=')
            if (firstPadding >= 0) {
                val paddingLength = encoded.length - firstPadding
                if (paddingLength !in 1..2 || encoded.substring(firstPadding).any { it != '=' }) {
                    return null
                }
            }
            val dataEnd = if (firstPadding >= 0) firstPadding else encoded.length
            if (encoded.substring(0, dataEnd).any { character ->
                    character !in 'A'..'Z' &&
                        character !in 'a'..'z' &&
                        character !in '0'..'9' &&
                        character != '+' &&
                        character != '/'
                }
            ) {
                return null
            }
            return try {
                val decoded = Base64.Default.decode(encoded)
                if (Base64.Default.encode(decoded) == encoded) {
                    decoded
                } else {
                    decoded.fill(0)
                    null
                }
            } catch (_: IllegalArgumentException) {
                null
            }
        }
    }
}

internal sealed interface CredentialEnvelopeState {
    data object Empty : CredentialEnvelopeState
    data object Malformed : CredentialEnvelopeState
    data class Supported(val envelope: CredentialEnvelope) : CredentialEnvelopeState
    data class Unsupported(val formatVersion: Int) : CredentialEnvelopeState
}
