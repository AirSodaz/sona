package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

data class StreamingProviderProfile(
    val providerId: String,
    val profileId: String,
    val streamingEndpoint: String,
    val streamingResourceId: String,
)

data class LocalSherpaStreamingConfig(
    val modelPath: String,
    val numThreads: Int,
    val modelType: String,
    val punctuationModel: String? = null,
    val vadModel: String? = null,
    val vadBuffer: Float = 5f,
    val fileConfig: LocalSherpaModelFiles? = null,
    val hotwords: String? = null,
    val gpuAcceleration: String? = null,
)

data class LocalSherpaModelFiles(
    val encoder: String? = null,
    val decoder: String? = null,
    val model: String? = null,
    val joiner: String? = null,
    val tokens: String? = null,
    val convFrontend: String? = null,
    val encoderAdaptor: String? = null,
    val llm: String? = null,
    val embedding: String? = null,
    val tokenizer: String? = null,
)

sealed interface StreamingEngineConfig {
    data class Online(
        val credential: StreamingCredential,
        val profile: StreamingProviderProfile,
    ) : StreamingEngineConfig

    data class LocalSherpa(
        val config: LocalSherpaStreamingConfig,
    ) : StreamingEngineConfig
}

data class StreamingTranscriptionRequest(
    val recordingId: String,
    val engine: StreamingEngineConfig,
    val language: String,
    val enableItn: Boolean,
)

sealed interface StreamingTranscriptionEvent {
    data class Transcript(
        val update: TranscriptUpdate,
    ) : StreamingTranscriptionEvent

    data class Failure(
        val code: String,
        val message: String,
    ) : StreamingTranscriptionEvent
}

fun interface StreamingProviderCatalogPort {
    suspend fun loadVolcengineStreamingProfile(): StreamingProviderProfile
}

interface StreamingTranscriptionPort {
    suspend fun open(request: StreamingTranscriptionRequest): StreamingTranscriptionSession
}

interface StreamingTranscriptionSession : AutoCloseable {
    val events: Flow<StreamingTranscriptionEvent>

    suspend fun start()
    suspend fun feed(frame: Pcm16Frame)
    suspend fun flush()
    suspend fun stop()
}
