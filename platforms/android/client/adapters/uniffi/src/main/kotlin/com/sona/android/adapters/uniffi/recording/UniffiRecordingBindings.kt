package com.sona.android.adapters.uniffi.recording

import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.createAsrStreamingSession
import uniffi.sona_uniffi_bind.completeHistoryLiveDraftJson
import uniffi.sona_uniffi_bind.createHistoryLiveDraftJson
import uniffi.sona_uniffi_bind.purgeHistoryItemsJson
import uniffi.sona_uniffi_bind.loadHistoryTranscriptJson
import uniffi.sona_uniffi_bind.queryHistoryWorkspaceJson
import uniffi.sona_uniffi_bind.findOnlineAsrProvider
import uniffi.sona_uniffi_bind.onlineAsrProviderRequest
import uniffi.sona_uniffi_bind.updateHistoryTranscriptJson
import uniffi.sona_uniffi_bind.volcengineDoubaoAsrConfigFromJson

internal data class UniffiStreamingProviderManifest(
    val providerId: String,
    val profileId: String,
    val defaultsJson: String,
    val streamingSupported: Boolean?,
)

internal data class UniffiVolcengineStreamingConfig(
    val streamingEndpoint: String,
    val streamingResourceId: String,
)

internal interface UniffiProviderBindings {
    fun findProvider(providerId: String): UniffiStreamingProviderManifest?
    fun parseVolcengineConfig(configJson: String): UniffiVolcengineStreamingConfig
}

internal object GeneratedUniffiProviderBindings : UniffiProviderBindings {
    override fun findProvider(providerId: String): UniffiStreamingProviderManifest? =
        findOnlineAsrProvider(providerId)?.let { provider ->
            UniffiStreamingProviderManifest(
                providerId = provider.id,
                profileId = provider.profileId,
                defaultsJson = provider.defaultsJson,
                streamingSupported = provider.streaming.supported,
            )
        }

    override fun parseVolcengineConfig(configJson: String): UniffiVolcengineStreamingConfig =
        volcengineDoubaoAsrConfigFromJson(configJson).let { config ->
            UniffiVolcengineStreamingConfig(
                streamingEndpoint = config.streamingEndpoint,
                streamingResourceId = config.streamingResourceId,
            )
        }
}

internal data class UniffiOnlineProviderRequest(
    val providerId: String,
    val profileId: String,
    val configJson: String,
)

internal interface UniffiStreamingSessionHandle : AutoCloseable {
    suspend fun start()
    suspend fun feedAudioChunk(bytes: ByteArray)
    suspend fun flush()
    suspend fun stop()
}

internal interface UniffiStreamingBindings {
    fun resolveProviderRequest(
        providerId: String,
        profileId: String,
        configJson: String,
    ): UniffiOnlineProviderRequest

    suspend fun createSession(
        instanceId: String,
        requestJson: String,
        observer: FfiAsrStreamingObserver,
    ): UniffiStreamingSessionHandle
}

internal object GeneratedUniffiStreamingBindings : UniffiStreamingBindings {
    override fun resolveProviderRequest(
        providerId: String,
        profileId: String,
        configJson: String,
    ): UniffiOnlineProviderRequest = onlineAsrProviderRequest(
        providerId = providerId,
        profileId = profileId,
        configJson = configJson,
    ).let { request ->
        UniffiOnlineProviderRequest(
            providerId = request.providerId,
            profileId = request.profileId,
            configJson = request.configJson,
        )
    }

    override suspend fun createSession(
        instanceId: String,
        requestJson: String,
        observer: FfiAsrStreamingObserver,
    ): UniffiStreamingSessionHandle = GeneratedUniffiStreamingSessionHandle(
        createAsrStreamingSession(instanceId, requestJson, observer),
    )
}

private class GeneratedUniffiStreamingSessionHandle(
    private val session: FfiAsrStreamingSession,
) : UniffiStreamingSessionHandle {
    override suspend fun start() = session.start()

    override suspend fun feedAudioChunk(bytes: ByteArray) = session.feedAudioChunk(bytes)

    override suspend fun flush() = session.flush()

    override suspend fun stop() = session.stop()

    override fun close() = session.close()
}

internal interface UniffiHistoryBindings {
    suspend fun createLiveDraft(appDataDir: String, requestJson: String): String
    suspend fun updateTranscript(appDataDir: String, requestJson: String): String
    suspend fun completeLiveDraft(appDataDir: String, requestJson: String): String
    suspend fun purgeItems(appDataDir: String, requestJson: String): String
    suspend fun queryWorkspace(appDataDir: String, requestJson: String): String
    suspend fun loadTranscript(appDataDir: String, historyId: String): String
}

internal object GeneratedUniffiHistoryBindings : UniffiHistoryBindings {
    override suspend fun createLiveDraft(appDataDir: String, requestJson: String): String =
        createHistoryLiveDraftJson(appDataDir, requestJson)

    override suspend fun updateTranscript(appDataDir: String, requestJson: String): String =
        updateHistoryTranscriptJson(appDataDir, requestJson)

    override suspend fun completeLiveDraft(appDataDir: String, requestJson: String): String =
        completeHistoryLiveDraftJson(appDataDir, requestJson)

    override suspend fun purgeItems(appDataDir: String, requestJson: String): String =
        purgeHistoryItemsJson(appDataDir, requestJson)

    override suspend fun queryWorkspace(appDataDir: String, requestJson: String): String =
        queryHistoryWorkspaceJson(appDataDir, requestJson)

    override suspend fun loadTranscript(appDataDir: String, historyId: String): String =
        loadHistoryTranscriptJson(appDataDir, historyId)
}
