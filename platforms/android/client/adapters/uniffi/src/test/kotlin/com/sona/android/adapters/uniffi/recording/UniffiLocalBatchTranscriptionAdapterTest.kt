package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.LocalBatchTranscriptionRequest
import com.sona.android.application.recording.LocalSherpaConfig
import com.sona.android.application.recording.LocalSherpaModelFiles
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiLocalAsrBatchRequest
import uniffi.sona_uniffi_bind.FfiLocalAsrBatchResult

class UniffiLocalBatchTranscriptionAdapterTest {
    @Test
    fun `maps the complete local model configuration into the generated request`() = runTest {
        lateinit var captured: FfiLocalAsrBatchRequest
        val adapter = UniffiLocalBatchTranscriptionAdapter { request ->
            captured = request
            FfiLocalAsrBatchResult(emptyList())
        }

        val result = adapter.transcribe(
            LocalBatchTranscriptionRequest(
                audioPath = "/jobs/input.wav",
                config = LocalSherpaConfig(
                    modelPath = "/models/qwen",
                    numThreads = 4,
                    modelType = "qwen3-asr",
                    punctuationModel = "/models/punctuation.onnx",
                    vadModel = "/models/silero_vad.onnx",
                    vadBuffer = 7f,
                    fileConfig = LocalSherpaModelFiles(
                        encoder = "encoder.int8.onnx",
                        decoder = "decoder.int8.onnx",
                        convFrontend = "conv_frontend.onnx",
                        tokenizer = "tokenizer",
                    ),
                    hotwords = "Sona",
                    gpuAcceleration = "cpu",
                ),
                language = "auto",
                enableItn = true,
            ),
        )

        assertTrue(result.segments.isEmpty())
        assertEquals("/jobs/input.wav", captured.audioPath)
        assertEquals("qwen3-asr", captured.modelType)
        assertEquals("conv_frontend.onnx", captured.files?.convFrontend)
        assertEquals("tokenizer", captured.files?.tokenizer)
        assertEquals("/models/silero_vad.onnx", captured.vadModel)
        assertEquals("/models/punctuation.onnx", captured.punctuationModel)
    }
}
