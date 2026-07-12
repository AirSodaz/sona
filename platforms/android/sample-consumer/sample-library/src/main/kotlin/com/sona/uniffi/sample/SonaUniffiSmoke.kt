package com.sona.uniffi.sample

import uniffi.sona_uniffi_bind.FfiAsrInferenceMetric
import uniffi.sona_uniffi_bind.FfiAsrModelLoadMetric
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.FfiLlmPromptChunk
import uniffi.sona_uniffi_bind.FfiPolishedSegment
import uniffi.sona_uniffi_bind.SonaCoreBindingException
import uniffi.sona_uniffi_bind.createOnlineAsrStreamingSession
import uniffi.sona_uniffi_bind.createProjectJson
import uniffi.sona_uniffi_bind.defaultConfigJson
import uniffi.sona_uniffi_bind.loadAutomationRepositoryStateJson
import uniffi.sona_uniffi_bind.loadAppConfigJson
import uniffi.sona_uniffi_bind.loadProjectRepositoryStateJson
import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson
import uniffi.sona_uniffi_bind.loadTaskLedgerSnapshotJson
import uniffi.sona_uniffi_bind.parsePolishChunkJson
import uniffi.sona_uniffi_bind.persistRecoveryQueueSnapshotJson
import uniffi.sona_uniffi_bind.planPolishPromptChunksJson
import uniffi.sona_uniffi_bind.saveRecoverySnapshotJson
import uniffi.sona_uniffi_bind.saveAppConfigJson
import uniffi.sona_uniffi_bind.upsertTaskLedgerRecordJson
import uniffi.sona_uniffi_bind.validateAutomationRuleActivationJson

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

data class SonaUniffiSmokeResult(
    val defaultConfigJson: String,
    val chunks: List<FfiLlmPromptChunk>,
    val polished: List<FfiPolishedSegment>,
)

object SonaUniffiSmoke {
    private val sampleSegmentsJson = """
        [
          {"id":"s1","text":"hello from android"},
          {"id":"s2","text":"next mobile segment"}
        ]
    """.trimIndent()

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

    fun createStreamingSession(): FfiAsrStreamingSession =
        createOnlineAsrStreamingSession(
            instanceId = "android-live-1",
            requestJson = streamingRequestJson,
            observer = RecordingAsrObserver(),
        )

    fun loadRecovery(appDataDir: String): String = loadRecoverySnapshotJson(appDataDir)

    fun loadTaskLedger(appDataDir: String): String = loadTaskLedgerSnapshotJson(appDataDir)

    fun loadAutomation(appDataDir: String): String =
        loadAutomationRepositoryStateJson(appDataDir)

    fun loadAppConfig(appDataDir: String): String? = loadAppConfigJson(appDataDir)

    fun saveAppConfig(appDataDir: String, configJson: String) =
        saveAppConfigJson(appDataDir, configJson)

    fun loadProjects(appDataDir: String): String = loadProjectRepositoryStateJson(appDataDir)

    fun createProject(appDataDir: String, inputJson: String): String =
        createProjectJson(appDataDir, inputJson)

    fun validateAutomation(
        ruleJson: String,
        globalConfigJson: String,
        projectJson: String?,
    ): String = validateAutomationRuleActivationJson(ruleJson, globalConfigJson, projectJson)

    fun upsertTaskLedger(appDataDir: String, recordJson: String): String =
        upsertTaskLedgerRecordJson(appDataDir, recordJson)

    fun saveRecovery(appDataDir: String, itemsJson: String): String =
        saveRecoverySnapshotJson(appDataDir, itemsJson)

    fun persistRecovery(
        appDataDir: String,
        queueItemsJson: String,
        resolvedIds: List<String>,
    ): String = persistRecoveryQueueSnapshotJson(appDataDir, queueItemsJson, resolvedIds)

    @Throws(SonaCoreBindingException::class)
    fun run(): SonaUniffiSmokeResult {
        val chunks = planPolishPromptChunksJson(
            segmentsJson = sampleSegmentsJson,
            context = "Android UniFFI smoke test",
            keywords = "Sona",
            chunkSize = 1UL,
            promptCharBudget = null,
        )
        val polished = parsePolishChunkJson(
            responseText = """
                {"id":"s1","text":"Hello from Android."}
            """.trimIndent(),
            expectedSegmentsJson = """
                [
                  {"id":"s1","text":"hello from android"}
                ]
            """.trimIndent(),
            chunkNumber = 1UL,
        )

        return SonaUniffiSmokeResult(
            defaultConfigJson = defaultConfigJson(),
            chunks = chunks,
            polished = polished,
        )
    }
}
