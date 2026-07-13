package com.sona.uniffi.consumer

import com.sona.uniffi.sample.SonaUniffiSmoke
import uniffi.sona_uniffi_bind.FfiAsrInferenceMetric
import uniffi.sona_uniffi_bind.FfiAsrModelLoadMetric
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.createOnlineAsrStreamingSession
import uniffi.sona_uniffi_bind.defaultConfigJson
import uniffi.sona_uniffi_bind.exportTranscriptFileJson
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

private class RecordingAsrObserver : FfiAsrStreamingObserver {
    private var latestTranscriptUpdate: FfiAsrTranscriptUpdateEvent? = null
    private var latestModelLoad: FfiAsrModelLoadMetric? = null
    private var latestLiveInference: FfiAsrInferenceMetric? = null

    override fun onTranscriptUpdate(event: FfiAsrTranscriptUpdateEvent) {
        latestTranscriptUpdate = event
    }

    override fun onModelLoad(metric: FfiAsrModelLoadMetric) {
        latestModelLoad = metric
    }

    override fun onLiveInference(metric: FfiAsrInferenceMetric) {
        latestLiveInference = metric
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

    fun loadProjects(appDataDir: String): String = loadProjectRepositoryStateJson(appDataDir)

    fun publishedSmokeTypeName(): String = SonaUniffiSmoke::class.java.name

    fun createStreamingSession(): FfiAsrStreamingSession =
        createOnlineAsrStreamingSession(
            instanceId = "android-live-1",
            requestJson = streamingRequestJson,
            observer = RecordingAsrObserver(),
        )
}
