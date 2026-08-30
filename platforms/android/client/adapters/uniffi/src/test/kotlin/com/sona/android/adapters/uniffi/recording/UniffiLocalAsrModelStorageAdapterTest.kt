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
}

private class FakeBindings : UniffiLocalAsrModelStorageBindings {
    var deletedModelId: String? = null
    var deletedModelsDir: String? = null

    override fun list(modelsDir: String, numThreads: UInt): List<FfiInstalledLocalAsrModel> =
        listOf(installedModel(numThreads))

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

private fun installedModel(numThreads: UInt) = FfiInstalledLocalAsrModel(
    id = "sensevoice",
    displayName = "SenseVoice Int8",
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
