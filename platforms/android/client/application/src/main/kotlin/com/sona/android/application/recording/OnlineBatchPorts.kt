package com.sona.android.application.recording

enum class OnlineBatchProvider {
    VOLCENGINE_DOUBAO,
    GROQ_WHISPER,
    MISTRAL_VOXTRAL,
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
    val provider: OnlineBatchProvider,
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
