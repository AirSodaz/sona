package com.sona.android.application.recording

enum class OnlineAsrProvider(
    val supportsStreaming: Boolean,
    val supportsBatch: Boolean,
) {
    VOLCENGINE_DOUBAO(supportsStreaming = true, supportsBatch = true),
    GROQ_WHISPER(supportsStreaming = false, supportsBatch = true),
    MISTRAL_VOXTRAL(supportsStreaming = false, supportsBatch = true),
    ;

    fun supports(mode: AsrMode): Boolean = when (mode) {
        AsrMode.STREAMING -> supportsStreaming
        AsrMode.BATCH -> supportsBatch
    }
}

class OnlineBatchCredential(
    val apiKey: String,
) {
    override fun equals(other: Any?): Boolean =
        other is OnlineBatchCredential && apiKey == other.apiKey

    override fun hashCode(): Int = apiKey.hashCode()

    override fun toString(): String = "OnlineBatchCredential(apiKey=<redacted>)"
}

data class OnlineBatchTranscriptionRequest(
    val audioPath: String,
    val provider: OnlineAsrProvider,
    val credential: OnlineBatchCredential,
    val language: String,
)

data class OnlineBatchTranscriptionResult(
    val segments: List<TranscriptSegment>,
    val audioDurationMillis: Double,
    val bufferedSamples: ULong,
    val stage: String,
)

fun interface OnlineBatchTranscriptionPort {
    suspend fun transcribe(request: OnlineBatchTranscriptionRequest): OnlineBatchTranscriptionResult
}
