package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.OnlineBatchCredential
import uniffi.sona_uniffi_bind.FfiOnlineAsrApiKey
import uniffi.sona_uniffi_bind.FfiOnlineAsrBatchProvider
import uniffi.sona_uniffi_bind.FfiOnlineAsrBatchRequest
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.transcribeOnlineAsrBatch

internal enum class UniffiOnlineBatchProvider {
    VOLCENGINE_DOUBAO,
    GROQ_WHISPER,
    MISTRAL_VOXTRAL,
}

internal data class UniffiOnlineBatchRequest(
    val audioPath: String,
    val provider: UniffiOnlineBatchProvider,
    val credential: OnlineBatchCredential,
    val language: String,
)

internal data class UniffiOnlineBatchResult(
    val segments: List<FfiTranscriptSegment>,
    val audioDurationMillis: Double,
    val bufferedSamples: ULong,
    val stage: String,
)

internal fun interface UniffiOnlineBatchBindings {
    suspend fun transcribe(request: UniffiOnlineBatchRequest): UniffiOnlineBatchResult
}

internal object GeneratedUniffiOnlineBatchBindings : UniffiOnlineBatchBindings {
    override suspend fun transcribe(request: UniffiOnlineBatchRequest): UniffiOnlineBatchResult {
        val apiKey = FfiOnlineAsrApiKey(request.credential.apiKey)
        return try {
            transcribeOnlineAsrBatch(
                FfiOnlineAsrBatchRequest(
                    audioPath = request.audioPath,
                    provider = request.provider.toGenerated(),
                    apiKey = apiKey,
                    language = request.language,
                ),
            ).let { result ->
                UniffiOnlineBatchResult(
                    segments = result.segments,
                    audioDurationMillis = result.audioDurationMs,
                    bufferedSamples = result.bufferedSamples,
                    stage = result.stage,
                )
            }
        } finally {
            apiKey.close()
        }
    }
}

private fun UniffiOnlineBatchProvider.toGenerated(): FfiOnlineAsrBatchProvider = when (this) {
    UniffiOnlineBatchProvider.VOLCENGINE_DOUBAO ->
        FfiOnlineAsrBatchProvider.VOLCENGINE_DOUBAO
    UniffiOnlineBatchProvider.GROQ_WHISPER -> FfiOnlineAsrBatchProvider.GROQ_WHISPER
    UniffiOnlineBatchProvider.MISTRAL_VOXTRAL -> FfiOnlineAsrBatchProvider.MISTRAL_VOXTRAL
}
