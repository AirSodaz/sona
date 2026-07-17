package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.OnlineBatchProvider
import com.sona.android.application.recording.OnlineBatchTranscriptionPort
import com.sona.android.application.recording.OnlineBatchTranscriptionRequest
import com.sona.android.application.recording.OnlineBatchTranscriptionResult
import uniffi.sona_uniffi_bind.FfiTranscriptSegment

class UniffiOnlineBatchTranscriptionAdapter internal constructor(
    private val bindings: UniffiOnlineBatchBindings,
) : OnlineBatchTranscriptionPort {
    constructor() : this(GeneratedUniffiOnlineBatchBindings)

    override suspend fun transcribe(
        request: OnlineBatchTranscriptionRequest,
    ): OnlineBatchTranscriptionResult {
        val result = bindings.transcribe(
            UniffiOnlineBatchRequest(
                audioPath = request.audioPath,
                provider = request.provider.toUniffi(),
                credential = request.credential,
                language = request.language,
            ),
        )
        return OnlineBatchTranscriptionResult(
            segments = result.segments.map(FfiTranscriptSegment::toApplication),
            audioDurationMillis = result.audioDurationMillis,
            bufferedSamples = result.bufferedSamples,
            stage = result.stage,
        )
    }
}

private fun OnlineBatchProvider.toUniffi(): UniffiOnlineBatchProvider = when (this) {
    OnlineBatchProvider.VOLCENGINE_DOUBAO -> UniffiOnlineBatchProvider.VOLCENGINE_DOUBAO
    OnlineBatchProvider.GROQ_WHISPER -> UniffiOnlineBatchProvider.GROQ_WHISPER
    OnlineBatchProvider.MISTRAL_VOXTRAL -> UniffiOnlineBatchProvider.MISTRAL_VOXTRAL
}
