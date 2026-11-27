package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.LocalAsrDownloadProgress
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrDownloadStage
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiInstalledLocalAsrModel
import uniffi.sona_uniffi_bind.FfiModelDownloadObserver
import uniffi.sona_uniffi_bind.FfiModelDownloadProgress
import uniffi.sona_uniffi_bind.FfiModelDownloadStage
import uniffi.sona_uniffi_bind.FfiModelFileConfig

class UniffiLocalAsrModelStorageAdapterTest {
    @Test
    fun `maps installed model and download progress from generated bindings`() = runTest {
        val bindings = FakeBindings()
        val adapter = UniffiLocalAsrModelStorageAdapter("/data/models", bindings)
        val progress = mutableListOf<LocalAsrDownloadProgress>()

        val listed = adapter.listInstalledModels(4).single()
        val downloaded = adapter.downloadModel(
            "sensevoice",
            4,
            LocalAsrDownloadProgressListener(progress::add),
        )

        assertEquals("sensevoice", listed.id)
        assertEquals("SenseVoice", listed.displayName)
        assertEquals("/data/models/sensevoice", listed.config.modelPath)
        assertEquals("model.int8.onnx", listed.config.fileConfig?.model)
        assertEquals("/data/models/silero_vad.onnx", listed.config.vadModel)
        assertEquals(setOf(AsrMode.STREAMING, AsrMode.BATCH), listed.supportedModes)
        assertEquals(listed, downloaded)
        assertEquals(LocalAsrDownloadStage.VERIFYING, progress.single().stage)
        assertEquals(12L, progress.single().downloadedBytes)
        assertTrue(adapter.validateModel("sensevoice"))

        adapter.deleteModel("sensevoice")
        assertEquals("sensevoice", bindings.deletedModelId)
        assertEquals("/data/models", bindings.deletedModelsDir)
    }

    @Test
    fun `installed catalog models resolve clean display names without int8`() {
        val adapter = UniffiLocalAsrModelStorageAdapter(
            "/data/models",
            FakeBindings(
                customModels = listOf(
                    installedModel(
                        numThreads = 2u,
                        id = "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
                        displayName = "Qwen3 ASR 0.6B Int8",
                    ),
                    installedModel(
                        numThreads = 2u,
                        id = "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27",
                        displayName = "Moonshine Tiny En Quantized",
                    ),
                ),
            ),
        )

        val models = adapter.listInstalledModels(2)
        assertEquals(listOf("Qwen3 ASR 0.6B", "Moonshine Tiny EN"), models.map { it.displayName })
    }
}

private class FakeBindings(
    private val customModels: List<FfiInstalledLocalAsrModel>? = null,
) : UniffiLocalAsrModelStorageBindings {
    var deletedModelId: String? = null
    var deletedModelsDir: String? = null

    override fun list(modelsDir: String, numThreads: UInt): List<FfiInstalledLocalAsrModel> =
        customModels ?: listOf(installedModel(numThreads))

    override suspend fun download(
        modelId: String,
        modelsDir: String,
        numThreads: UInt,
        observer: FfiModelDownloadObserver,
    ): FfiInstalledLocalAsrModel {
        observer.onProgress(
            FfiModelDownloadProgress(
                modelId = modelId,
                componentModelId = "silero-vad",
                stage = FfiModelDownloadStage.VERIFYING,
                downloadedBytes = 12uL,
                totalBytes = 20uL,
            ),
        )
        return installedModel(numThreads)
    }

    override suspend fun validate(modelId: String, modelsDir: String): Boolean = true

    override fun delete(modelId: String, modelsDir: String) {
        deletedModelId = modelId
        deletedModelsDir = modelsDir
    }
}

private fun installedModel(
    numThreads: UInt,
    id: String = "sensevoice",
    displayName: String = "SenseVoice Int8",
) = FfiInstalledLocalAsrModel(
    id = id,
    displayName = displayName,
    modelPath = "/data/models/sensevoice",
    modelType = "sensevoice",
    modes = listOf("streaming", "batch"),
    sizeBytes = 42uL,
    numThreads = numThreads,
    vadModelPath = "/data/models/silero_vad.onnx",
    punctuationModelPath = null,
    files = FfiModelFileConfig(
        encoder = null,
        decoder = null,
        model = "model.int8.onnx",
        joiner = null,
        tokens = "tokens.txt",
        convFrontend = null,
        encoderAdaptor = null,
        llm = null,
        embedding = null,
        tokenizer = null,
        mmproj = null,
        preprocessor = null,
        uncachedDecoder = null,
        cachedDecoder = null,
        mergedDecoder = null,
    ),
)
