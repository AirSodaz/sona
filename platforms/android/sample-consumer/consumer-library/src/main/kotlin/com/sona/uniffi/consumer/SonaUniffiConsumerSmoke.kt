package com.sona.uniffi.consumer

import com.sona.uniffi.sample.SonaUniffiSmoke
import uniffi.sona_uniffi_bind.FfiAsrInferenceMetric
import uniffi.sona_uniffi_bind.FfiAsrModelLoadMetric
import uniffi.sona_uniffi_bind.FfiAsrStreamingErrorEvent
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.createAsrStreamingSession
import uniffi.sona_uniffi_bind.completeHistoryLiveDraftJson
import uniffi.sona_uniffi_bind.createHistoryLiveDraftJson
import uniffi.sona_uniffi_bind.createHistoryTranscriptSnapshotJson
import uniffi.sona_uniffi_bind.defaultConfigJson
import uniffi.sona_uniffi_bind.deleteHistoryItemsJson
import uniffi.sona_uniffi_bind.exportBackupArchiveJson
import uniffi.sona_uniffi_bind.exportTranscriptFileJson
import uniffi.sona_uniffi_bind.importBackupArchiveJson
import uniffi.sona_uniffi_bind.inspectBackupArchiveJson
import uniffi.sona_uniffi_bind.loadAutomationRepositoryStateJson
import uniffi.sona_uniffi_bind.loadAppConfigJson
import uniffi.sona_uniffi_bind.loadDashboardSnapshotJson
import uniffi.sona_uniffi_bind.loadDiagnosticsSnapshotJson
import uniffi.sona_uniffi_bind.listHistoryItemsJson
import uniffi.sona_uniffi_bind.listHistoryTranscriptSnapshotsJson
import uniffi.sona_uniffi_bind.loadHistoryTranscriptJson
import uniffi.sona_uniffi_bind.loadHistoryTranscriptSnapshotJson
import uniffi.sona_uniffi_bind.loadProjectRepositoryStateJson
import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson
import uniffi.sona_uniffi_bind.loadStorageUsageSnapshotJson
import uniffi.sona_uniffi_bind.loadTaskLedgerSnapshotJson
import uniffi.sona_uniffi_bind.queryHistoryWorkspaceJson
import uniffi.sona_uniffi_bind.reassignHistoryProjectJson
import uniffi.sona_uniffi_bind.saveHistoryImportedFileJson
import uniffi.sona_uniffi_bind.saveHistoryRecordingJson
import uniffi.sona_uniffi_bind.updateHistoryItemMetaJson
import uniffi.sona_uniffi_bind.updateHistoryProjectAssignmentsJson
import uniffi.sona_uniffi_bind.updateHistoryTranscriptJson

private class RecordingAsrObserver : FfiAsrStreamingObserver {
    private var latestTranscriptUpdate: FfiAsrTranscriptUpdateEvent? = null
    private var latestModelLoad: FfiAsrModelLoadMetric? = null
    private var latestLiveInference: FfiAsrInferenceMetric? = null
    private var latestStreamingError: FfiAsrStreamingErrorEvent? = null

    override fun onTranscriptUpdate(event: FfiAsrTranscriptUpdateEvent) {
        latestTranscriptUpdate = event
    }

    override fun onModelLoad(metric: FfiAsrModelLoadMetric) {
        latestModelLoad = metric
    }

    override fun onLiveInference(metric: FfiAsrInferenceMetric) {
        latestLiveInference = metric
    }

    override fun onStreamingError(event: FfiAsrStreamingErrorEvent) {
        latestStreamingError = event
    }
}

object SonaUniffiConsumerSmoke {
    private val streamingRequestJson = """
        {
          "mode": "streaming",
          "language": "auto",
          "enableItn": false,
          "normalizationOptions": {"enableTimeline": false},
          "postprocessOptions": {
            "textReplacementSets": [],
            "dropFinalDotSegments": true
          },
          "engine": "online",
          "onlineProvider": {
            "providerId": "volcengine-doubao",
            "profileId": "volcengine-doubao-default",
            "config": {
              "apiKey": "android-smoke-key",
              "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
              "streamingResourceId": "volc.seedasr.sauc.duration"
            }
          }
        }
    """.trimIndent()

    fun defaultConfig(): String = defaultConfigJson()

    fun loadRecovery(appDataDir: String): String = loadRecoverySnapshotJson(appDataDir)

    fun loadTaskLedger(appDataDir: String): String = loadTaskLedgerSnapshotJson(appDataDir)

    fun loadAutomation(appDataDir: String): String =
        loadAutomationRepositoryStateJson(appDataDir)

    fun loadAppConfig(appDataDir: String): String? = loadAppConfigJson(appDataDir)

    suspend fun loadDashboard(appDataDir: String): String =
        loadDashboardSnapshotJson(appDataDir, false)

    suspend fun loadDiagnostics(appDataDir: String, inputJson: String): String =
        loadDiagnosticsSnapshotJson(appDataDir, inputJson)

    suspend fun loadStorageUsage(appDataDir: String): String =
        loadStorageUsageSnapshotJson(appDataDir)

    suspend fun exportTranscript(inputJson: String): String =
        exportTranscriptFileJson(inputJson)

    suspend fun exportBackupArchive(
        appDataDir: String,
        archivePath: String,
        appVersion: String,
    ): String = exportBackupArchiveJson(appDataDir, archivePath, appVersion)

    suspend fun inspectBackupArchive(archivePath: String): String =
        inspectBackupArchiveJson(archivePath)

    suspend fun importBackupArchive(
        appDataDir: String,
        archivePath: String,
        defaultRuleSetName: String,
        confirmReplace: Boolean,
    ): String = importBackupArchiveJson(
        appDataDir,
        archivePath,
        defaultRuleSetName,
        confirmReplace,
    )

    suspend fun listHistory(appDataDir: String, limit: ULong?, offset: ULong?): String =
        listHistoryItemsJson(appDataDir, limit, offset)

    suspend fun queryHistory(appDataDir: String, requestJson: String): String =
        queryHistoryWorkspaceJson(appDataDir, requestJson)

    suspend fun loadHistoryTranscript(appDataDir: String, historyId: String): String =
        loadHistoryTranscriptJson(appDataDir, historyId)

    suspend fun listHistorySnapshots(appDataDir: String, historyId: String): String =
        listHistoryTranscriptSnapshotsJson(appDataDir, historyId)

    suspend fun loadHistorySnapshot(
        appDataDir: String,
        historyId: String,
        snapshotId: String,
    ): String = loadHistoryTranscriptSnapshotJson(appDataDir, historyId, snapshotId)

    suspend fun createHistoryLiveDraft(appDataDir: String, requestJson: String): String =
        createHistoryLiveDraftJson(appDataDir, requestJson)

    suspend fun completeHistoryLiveDraft(appDataDir: String, requestJson: String): String =
        completeHistoryLiveDraftJson(appDataDir, requestJson)

    suspend fun saveHistoryRecording(
        appDataDir: String,
        requestJson: String,
        audioBytes: ByteArray?,
        nativeAudioPath: String?,
    ): String = saveHistoryRecordingJson(appDataDir, requestJson, audioBytes, nativeAudioPath)

    suspend fun saveHistoryImportedFile(appDataDir: String, requestJson: String): String =
        saveHistoryImportedFileJson(appDataDir, requestJson)

    suspend fun deleteHistoryItems(appDataDir: String, requestJson: String): String =
        deleteHistoryItemsJson(appDataDir, requestJson)

    suspend fun updateHistoryTranscript(appDataDir: String, requestJson: String): String =
        updateHistoryTranscriptJson(appDataDir, requestJson)

    suspend fun createHistoryTranscriptSnapshot(
        appDataDir: String,
        requestJson: String,
    ): String = createHistoryTranscriptSnapshotJson(appDataDir, requestJson)

    suspend fun updateHistoryItemMeta(appDataDir: String, requestJson: String): String =
        updateHistoryItemMetaJson(appDataDir, requestJson)

    suspend fun updateHistoryProjectAssignments(
        appDataDir: String,
        requestJson: String,
    ): String = updateHistoryProjectAssignmentsJson(appDataDir, requestJson)

    suspend fun reassignHistoryProject(appDataDir: String, requestJson: String): String =
        reassignHistoryProjectJson(appDataDir, requestJson)

    fun loadProjects(appDataDir: String): String = loadProjectRepositoryStateJson(appDataDir)

    fun publishedSmokeTypeName(): String = SonaUniffiSmoke::class.java.name

    suspend fun createStreamingSession(): FfiAsrStreamingSession =
        createAsrStreamingSession(
            instanceId = "android-live-1",
            requestJson = streamingRequestJson,
            observer = RecordingAsrObserver(),
        )
}
