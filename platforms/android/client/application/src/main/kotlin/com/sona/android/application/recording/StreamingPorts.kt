package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

data class StreamingProviderProfile(
    val providerId: String,
    val profileId: String,
    val streamingEndpoint: String,
    val streamingResourceId: String,
)

data class StreamingTranscriptionRequest(
    val recordingId: String,
    val credential: StreamingCredential,
    val profile: StreamingProviderProfile,
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
