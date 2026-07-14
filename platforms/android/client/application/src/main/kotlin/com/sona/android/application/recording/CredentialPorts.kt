package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

class StreamingCredential(
    val apiKey: String,
) {
    override fun equals(other: Any?): Boolean =
        other is StreamingCredential && apiKey == other.apiKey

    override fun hashCode(): Int = apiKey.hashCode()

    override fun toString(): String = "StreamingCredential(apiKey=<redacted>)"
}

enum class CredentialStatus {
    NOT_CONFIGURED,
    CONFIGURED,
}

interface StreamingCredentialSettingsPort {
    val status: Flow<CredentialStatus>

    suspend fun save(credential: StreamingCredential)
    suspend fun clear()
}

fun interface StreamingCredentialResolverPort {
    suspend fun loadForStart(): StreamingCredential?
}
