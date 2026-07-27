package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

/**
 * The provider that cloud batch transcription runs against, together with the
 * secret it needs. Resolved only when a transcription actually starts.
 */
data class ActiveBatchCredential(
    val provider: OnlineBatchProvider,
    val credential: OnlineBatchCredential,
)

/**
 * Everything the settings surface may know about stored batch credentials:
 * which provider is active and which providers hold a key. Never the keys.
 */
data class BatchCredentialConfiguration(
    val selectedProvider: OnlineBatchProvider = OnlineBatchProvider.VOLCENGINE_DOUBAO,
    val configuredProviders: Set<OnlineBatchProvider> = emptySet(),
) {
    val selectedStatus: CredentialStatus
        get() = statusFor(selectedProvider)

    fun statusFor(provider: OnlineBatchProvider): CredentialStatus =
        if (provider in configuredProviders) {
            CredentialStatus.CONFIGURED
        } else {
            CredentialStatus.NOT_CONFIGURED
        }
}

interface BatchCredentialSettingsPort {
    val configuration: Flow<BatchCredentialConfiguration>

    suspend fun selectProvider(provider: OnlineBatchProvider)

    suspend fun save(provider: OnlineBatchProvider, credential: OnlineBatchCredential)

    suspend fun clear(provider: OnlineBatchProvider)
}

fun interface BatchCredentialResolverPort {
    suspend fun loadActive(): ActiveBatchCredential?
}
