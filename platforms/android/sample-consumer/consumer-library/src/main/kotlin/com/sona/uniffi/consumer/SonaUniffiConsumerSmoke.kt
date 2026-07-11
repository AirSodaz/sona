package com.sona.uniffi.consumer

import com.sona.uniffi.sample.SonaUniffiSmoke
import uniffi.sona_uniffi_bind.FfiAsrInferenceMetric
import uniffi.sona_uniffi_bind.FfiAsrModelLoadMetric
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.createOnlineAsrStreamingSession
import uniffi.sona_uniffi_bind.defaultConfigJson
import uniffi.sona_uniffi_bind.loadRecoverySnapshotJson

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

    fun publishedSmokeTypeName(): String = SonaUniffiSmoke::class.java.name

    fun createStreamingSession(): FfiAsrStreamingSession =
        createOnlineAsrStreamingSession(
            instanceId = "android-live-1",
            requestJson = streamingRequestJson,
            observer = RecordingAsrObserver(),
        )
}
