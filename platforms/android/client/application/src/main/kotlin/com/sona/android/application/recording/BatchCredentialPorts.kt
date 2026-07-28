package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

enum class CredentialStatus {
    NOT_CONFIGURED,
    CONFIGURED,
}

/**
 * The provider that cloud batch transcription runs against, together with the
 * secret it needs. Resolved only when a transcription actually starts.
 */
data class ActiveBatchCredential(
    val provider: OnlineAsrProvider,
    val credential: OnlineBatchCredential,
)

/**
 * Everything the settings surface may know about stored batch credentials:
 * which provider is active and which providers hold a key. Never the keys.
 */
data class BatchCredentialConfiguration(
    val selectedProvider: OnlineAsrProvider = OnlineAsrProvider.VOLCENGINE_DOUBAO,
    val configuredProviders: Set<OnlineAsrProvider> = emptySet(),
) {
    val selectedStatus: CredentialStatus
        get() = statusFor(selectedProvider)

    fun statusFor(provider: OnlineAsrProvider): CredentialStatus =
        if (provider in configuredProviders) {
            CredentialStatus.CONFIGURED
        } else {
            CredentialStatus.NOT_CONFIGURED
        }
}

interface BatchCredentialSettingsPort {
    val configuration: Flow<BatchCredentialConfiguration>

    suspend fun selectProvider(provider: OnlineAsrProvider)

    suspend fun save(provider: OnlineAsrProvider, credential: OnlineBatchCredential)

    suspend fun clear(provider: OnlineAsrProvider)
}

fun interface BatchCredentialResolverPort {
    suspend fun loadActive(): ActiveBatchCredential?

    suspend fun load(provider: OnlineAsrProvider): OnlineBatchCredential? =
        loadActive()?.takeIf { it.provider == provider }?.credential
}
