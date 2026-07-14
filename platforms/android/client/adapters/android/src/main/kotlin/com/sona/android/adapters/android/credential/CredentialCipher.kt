package com.sona.android.adapters.android.credential

import kotlinx.coroutines.flow.Flow

internal interface CredentialStore {
    val records: Flow<CredentialRecord>

    suspend fun read(): CredentialRecord
    suspend fun write(record: CredentialRecord)
    suspend fun clear()
}

internal interface CredentialCipher {
    fun encrypt(plaintext: ByteArray): CredentialEnvelope
    fun decrypt(envelope: CredentialEnvelope): ByteArray
    fun deleteKey()
}

internal enum class CredentialCipherFailureKind {
    PERMANENT,
    TEMPORARY,
}

internal class CredentialCipherException(
    val kind: CredentialCipherFailureKind,
) : Exception(
    when (kind) {
        CredentialCipherFailureKind.PERMANENT -> "Credential key is permanently unavailable."
        CredentialCipherFailureKind.TEMPORARY -> "Credential cipher is temporarily unavailable."
    },
)
