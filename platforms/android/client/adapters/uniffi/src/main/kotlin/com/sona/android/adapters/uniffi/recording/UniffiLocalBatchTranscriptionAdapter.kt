package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.LocalBatchTranscriptionPort
import com.sona.android.application.recording.LocalBatchTranscriptionRequest
import com.sona.android.application.recording.LocalBatchTranscriptionResult
import com.sona.android.application.recording.LocalSherpaModelFiles
import uniffi.sona_uniffi_bind.FfiLocalAsrBatchRequest
import uniffi.sona_uniffi_bind.FfiLocalAsrBatchResult
import uniffi.sona_uniffi_bind.FfiLocalAsrModelFiles
import uniffi.sona_uniffi_bind.transcribeLocalAsrBatch

class UniffiLocalBatchTranscriptionAdapter internal constructor(
    private val transcribe: suspend (FfiLocalAsrBatchRequest) -> FfiLocalAsrBatchResult,
) : LocalBatchTranscriptionPort {
    constructor() : this(::transcribeLocalAsrBatch)

    override suspend fun transcribe(
        request: LocalBatchTranscriptionRequest,
    ): LocalBatchTranscriptionResult {
        val config = request.config
        val result = transcribe(
            FfiLocalAsrBatchRequest(
                audioPath = request.audioPath,
                modelPath = config.modelPath,
                numThreads = config.numThreads,
                modelType = config.modelType,
                punctuationModel = config.punctuationModel,
                vadModel = config.vadModel,
                vadBuffer = config.vadBuffer,
                files = config.fileConfig?.toUniffi(),
                language = request.language,
                enableItn = request.enableItn,
                hotwords = config.hotwords,
                gpuAcceleration = config.gpuAcceleration,
            ),
        )
        return LocalBatchTranscriptionResult(result.segments.map { it.toApplication() })
    }
}

private fun LocalSherpaModelFiles.toUniffi() = FfiLocalAsrModelFiles(
    encoder = encoder,
    decoder = decoder,
    model = model,
    joiner = joiner,
    tokens = tokens,
    convFrontend = convFrontend,
    encoderAdaptor = encoderAdaptor,
    llm = llm,
    embedding = embedding,
    tokenizer = tokenizer,
)
