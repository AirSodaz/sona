package com.sona.android.adapters.uniffi.recording

import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.FfiHistoryCompleteLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryCreateLiveDraftRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryDeleteItemsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryItemRecordV1
import uniffi.sona_uniffi_bind.FfiHistorySaveImportedFileRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateTranscriptRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceQueryRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryWorkspaceQueryResultV1
import uniffi.sona_uniffi_bind.FfiHistoryTrashItemsRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateItemMetaRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryUpdateTagAssignmentsRequestV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotMetadataV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotRecordV1
import uniffi.sona_uniffi_bind.FfiLiveRecordingDraftResultV1
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.completeHistoryLiveDraftV1
import uniffi.sona_uniffi_bind.createAsrStreamingSession
import uniffi.sona_uniffi_bind.createHistoryLiveDraftV1
import uniffi.sona_uniffi_bind.findOnlineAsrProvider
import uniffi.sona_uniffi_bind.loadHistoryTranscriptV1
import uniffi.sona_uniffi_bind.listHistoryTranscriptSnapshotsV1
import uniffi.sona_uniffi_bind.loadHistoryTranscriptSnapshotV1
import uniffi.sona_uniffi_bind.onlineAsrProviderRequest
import uniffi.sona_uniffi_bind.purgeHistoryItemsV1
import uniffi.sona_uniffi_bind.queryHistoryWorkspaceV1
import uniffi.sona_uniffi_bind.saveHistoryImportedFileV1
import uniffi.sona_uniffi_bind.restoreHistoryItemsV1
import uniffi.sona_uniffi_bind.trashHistoryItemsV1
import uniffi.sona_uniffi_bind.updateHistoryItemMetaV1
import uniffi.sona_uniffi_bind.updateHistoryTagAssignmentsV1
import uniffi.sona_uniffi_bind.updateHistoryTranscriptV1
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
    suspend fun feedPcm16Frame(sequence: ULong, startSample: ULong, bytes: ByteArray)
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

    override suspend fun feedPcm16Frame(sequence: ULong, startSample: ULong, bytes: ByteArray) =
        session.feedPcm16Frame(sequence, startSample, bytes)

    override suspend fun flush() = session.flush()

    override suspend fun stop() = session.stop()

    override fun close() = session.close()
}

internal interface UniffiHistoryBindings {
    suspend fun createLiveDraft(
        appDataDir: String,
        request: FfiHistoryCreateLiveDraftRequestV1,
    ): FfiLiveRecordingDraftResultV1

    suspend fun updateTranscript(
        appDataDir: String,
        request: FfiHistoryUpdateTranscriptRequestV1,
    ): FfiHistoryItemRecordV1

    suspend fun completeLiveDraft(
        appDataDir: String,
        request: FfiHistoryCompleteLiveDraftRequestV1,
    ): FfiHistoryItemRecordV1

    suspend fun purgeItems(appDataDir: String, request: FfiHistoryDeleteItemsRequestV1)

    suspend fun queryWorkspace(
        appDataDir: String,
        request: FfiHistoryWorkspaceQueryRequestV1,
    ): FfiHistoryWorkspaceQueryResultV1

    suspend fun loadTranscript(
        appDataDir: String,
        historyId: String,
    ): List<FfiTranscriptSegment>?

    suspend fun updateItemMeta(appDataDir: String, request: FfiHistoryUpdateItemMetaRequestV1)
    suspend fun updateTagAssignments(appDataDir: String, request: FfiHistoryUpdateTagAssignmentsRequestV1)
    suspend fun trashItems(appDataDir: String, request: FfiHistoryTrashItemsRequestV1)
    suspend fun restoreItems(appDataDir: String, request: FfiHistoryDeleteItemsRequestV1)
    suspend fun listSnapshots(appDataDir: String, historyId: String): List<FfiTranscriptSnapshotMetadataV1>
    suspend fun loadSnapshot(appDataDir: String, historyId: String, snapshotId: String): FfiTranscriptSnapshotRecordV1?

    suspend fun saveImported(
        appDataDir: String,
        request: FfiHistorySaveImportedFileRequestV1,
    ): FfiHistoryItemRecordV1 = throw UnsupportedOperationException("Imported history is unavailable.")
}

internal object GeneratedUniffiHistoryBindings : UniffiHistoryBindings {
    override suspend fun createLiveDraft(
        appDataDir: String,
        request: FfiHistoryCreateLiveDraftRequestV1,
    ): FfiLiveRecordingDraftResultV1 = createHistoryLiveDraftV1(appDataDir, request)

    override suspend fun updateTranscript(
        appDataDir: String,
        request: FfiHistoryUpdateTranscriptRequestV1,
    ): FfiHistoryItemRecordV1 = updateHistoryTranscriptV1(appDataDir, request)

    override suspend fun completeLiveDraft(
        appDataDir: String,
        request: FfiHistoryCompleteLiveDraftRequestV1,
    ): FfiHistoryItemRecordV1 = completeHistoryLiveDraftV1(appDataDir, request)

    override suspend fun purgeItems(
        appDataDir: String,
        request: FfiHistoryDeleteItemsRequestV1,
    ) = purgeHistoryItemsV1(appDataDir, request)

    override suspend fun queryWorkspace(
        appDataDir: String,
        request: FfiHistoryWorkspaceQueryRequestV1,
    ): FfiHistoryWorkspaceQueryResultV1 = queryHistoryWorkspaceV1(appDataDir, request)

    override suspend fun loadTranscript(
        appDataDir: String,
        historyId: String,
    ): List<FfiTranscriptSegment>? = loadHistoryTranscriptV1(appDataDir, historyId)

    override suspend fun updateItemMeta(
        appDataDir: String,
        request: FfiHistoryUpdateItemMetaRequestV1,
    ) = updateHistoryItemMetaV1(appDataDir, request)

    override suspend fun updateTagAssignments(
        appDataDir: String,
        request: FfiHistoryUpdateTagAssignmentsRequestV1,
    ) = updateHistoryTagAssignmentsV1(appDataDir, request)

    override suspend fun trashItems(
        appDataDir: String,
        request: FfiHistoryTrashItemsRequestV1,
    ) = trashHistoryItemsV1(appDataDir, request)

    override suspend fun restoreItems(
        appDataDir: String,
        request: FfiHistoryDeleteItemsRequestV1,
    ) = restoreHistoryItemsV1(appDataDir, request)

    override suspend fun listSnapshots(
        appDataDir: String,
        historyId: String,
    ): List<FfiTranscriptSnapshotMetadataV1> = listHistoryTranscriptSnapshotsV1(appDataDir, historyId)

    override suspend fun loadSnapshot(
        appDataDir: String,
        historyId: String,
        snapshotId: String,
    ): FfiTranscriptSnapshotRecordV1? = loadHistoryTranscriptSnapshotV1(appDataDir, historyId, snapshotId)

    override suspend fun saveImported(
        appDataDir: String,
        request: FfiHistorySaveImportedFileRequestV1,
    ): FfiHistoryItemRecordV1 = saveHistoryImportedFileV1(appDataDir, request)
}
