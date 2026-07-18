package com.sona.android.adapters.android.sync

internal class SyncSecretEnvelope(
    iv: ByteArray,
    ciphertext: ByteArray,
) {
    private val ivBytes = iv.copyOf()
    private val ciphertextBytes = ciphertext.copyOf()

    val iv: ByteArray
        get() = ivBytes.copyOf()

    val ciphertext: ByteArray
        get() = ciphertextBytes.copyOf()

    init {
        require(iv.size == IV_SIZE_BYTES) { "Sync secret envelope IV has an invalid size." }
        require(ciphertext.size >= GCM_TAG_SIZE_BYTES) {
            "Sync secret envelope ciphertext has an invalid size."
        }
    }

    override fun toString(): String = "SyncSecretEnvelope(iv=<redacted>, ciphertext=<redacted>)"

    companion object {
        const val IV_SIZE_BYTES = 12
        const val GCM_TAG_SIZE_BYTES = 16

        fun fromTemporaryBuffers(
            iv: ByteArray,
            ciphertext: ByteArray,
        ): SyncSecretEnvelope = try {
            SyncSecretEnvelope(iv, ciphertext)
        } finally {
            iv.fill(0)
            ciphertext.fill(0)
        }
    }
}

internal interface SyncSecretRecordStore {
    suspend fun get(logicalKey: String): SyncSecretEnvelope?

    suspend fun set(logicalKey: String, envelope: SyncSecretEnvelope)

    suspend fun delete(logicalKey: String)

    suspend fun clear()
}

internal interface SyncSecretCipher {
    fun encrypt(logicalKey: String, plaintext: ByteArray): SyncSecretEnvelope

    fun decrypt(logicalKey: String, envelope: SyncSecretEnvelope): ByteArray

    fun resetAfterKeyInvalidation()
}

internal enum class SyncSecretCipherFailureKind {
    PERMANENT_RECORD,
    PERMANENT_KEY,
    TEMPORARY,
}

internal class SyncSecretCipherException(
    val kind: SyncSecretCipherFailureKind,
) : Exception(
    when (kind) {
        SyncSecretCipherFailureKind.PERMANENT_RECORD ->
            "Sync secret record is permanently unreadable."
        SyncSecretCipherFailureKind.PERMANENT_KEY ->
            "Sync secret key is permanently unavailable."
        SyncSecretCipherFailureKind.TEMPORARY -> "Sync secret cipher is temporarily unavailable."
    },
)
