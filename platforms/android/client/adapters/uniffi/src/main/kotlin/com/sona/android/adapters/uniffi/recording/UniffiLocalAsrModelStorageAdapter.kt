package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.LocalAsrDownloadProgress
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrDownloadStage
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalAsrModelSource
import com.sona.android.application.recording.LocalAsrModelStoragePort
import com.sona.android.application.recording.LocalSherpaConfig
import com.sona.android.application.recording.LocalSherpaModelFiles
import uniffi.sona_uniffi_bind.FfiInstalledLocalAsrModel
import uniffi.sona_uniffi_bind.FfiModelDownloadObserver
import uniffi.sona_uniffi_bind.FfiModelDownloadProgress
import uniffi.sona_uniffi_bind.FfiModelDownloadStage
import uniffi.sona_uniffi_bind.deleteLocalAsrModel
import uniffi.sona_uniffi_bind.downloadLocalAsrModel
import uniffi.sona_uniffi_bind.listInstalledLocalAsrModels
import uniffi.sona_uniffi_bind.validateLocalAsrModel

internal interface UniffiLocalAsrModelStorageBindings {
    fun list(modelsDir: String, numThreads: UInt): List<FfiInstalledLocalAsrModel>

    suspend fun download(
        modelId: String,
        modelsDir: String,
        numThreads: UInt,
        observer: FfiModelDownloadObserver,
    ): FfiInstalledLocalAsrModel

    suspend fun validate(modelId: String, modelsDir: String): Boolean

    fun delete(modelId: String, modelsDir: String)
}

internal object GeneratedUniffiLocalAsrModelStorageBindings : UniffiLocalAsrModelStorageBindings {
    override fun list(modelsDir: String, numThreads: UInt): List<FfiInstalledLocalAsrModel> =
        listInstalledLocalAsrModels(modelsDir, numThreads)

    override suspend fun download(
        modelId: String,
        modelsDir: String,
        numThreads: UInt,
        observer: FfiModelDownloadObserver,
    ): FfiInstalledLocalAsrModel =
        downloadLocalAsrModel(modelId, modelsDir, numThreads, observer)

    override suspend fun validate(modelId: String, modelsDir: String): Boolean =
        validateLocalAsrModel(modelId, modelsDir)

    override fun delete(modelId: String, modelsDir: String) {
        deleteLocalAsrModel(modelId, modelsDir)
    }
}

class UniffiLocalAsrModelStorageAdapter internal constructor(
    private val modelsDir: String,
    private val bindings: UniffiLocalAsrModelStorageBindings,
) : LocalAsrModelStoragePort {
    constructor(modelsDir: String) : this(modelsDir, GeneratedUniffiLocalAsrModelStorageBindings)

    init {
        require(modelsDir.isNotBlank()) { "Models directory must not be blank." }
    }

    override fun listInstalledModels(numThreads: Int): List<LocalAsrModel> =
        bindings.list(modelsDir, numThreads.toBindingThreads())
            .map(FfiInstalledLocalAsrModel::toApplication)

    override suspend fun downloadModel(
        modelId: String,
        numThreads: Int,
        progress: LocalAsrDownloadProgressListener,
    ): LocalAsrModel = bindings.download(
        modelId = modelId,
        modelsDir = modelsDir,
        numThreads = numThreads.toBindingThreads(),
        observer = object : FfiModelDownloadObserver {
            override fun onProgress(event: FfiModelDownloadProgress) {
                progress.onProgress(event.toApplication())
            }
        },
    ).toApplication()

    override suspend fun validateModel(modelId: String): Boolean =
        bindings.validate(modelId, modelsDir)

    override suspend fun deleteModel(modelId: String) {
        bindings.delete(modelId, modelsDir)
    }
}

internal fun FfiInstalledLocalAsrModel.toApplication(): LocalAsrModel = LocalAsrModel(
    id = id,
    displayName = displayName,
    config = LocalSherpaConfig(
        modelPath = modelPath,
        numThreads = numThreads.toInt(),
        modelType = modelType,
        punctuationModel = punctuationModelPath,
        vadModel = vadModelPath,
        fileConfig = LocalSherpaModelFiles(
            encoder = files.encoder,
            decoder = files.decoder,
            model = files.model,
            joiner = files.joiner,
            tokens = files.tokens,
            convFrontend = files.convFrontend,
            encoderAdaptor = files.encoderAdaptor,
            llm = files.llm,
            embedding = files.embedding,
            tokenizer = files.tokenizer,
            mmproj = files.mmproj,
            preprocessor = files.preprocessor,
            uncachedDecoder = files.uncachedDecoder,
            cachedDecoder = files.cachedDecoder,
            mergedDecoder = files.mergedDecoder,
        ),
    ),
    supportedModes = modes.mapNotNullTo(mutableSetOf()) {
        when (it) {
            "streaming" -> AsrMode.STREAMING
            "batch" -> AsrMode.BATCH
            else -> null
        }
    },
    sizeBytes = sizeBytes.toLongSaturated(),
    source = LocalAsrModelSource.CATALOG,
)

internal fun FfiModelDownloadProgress.toApplication(): LocalAsrDownloadProgress =
    LocalAsrDownloadProgress(
        modelId = modelId,
        stage = when (stage) {
            FfiModelDownloadStage.DOWNLOADING -> LocalAsrDownloadStage.DOWNLOADING
            FfiModelDownloadStage.VERIFYING -> LocalAsrDownloadStage.VERIFYING
            FfiModelDownloadStage.INSTALLING -> LocalAsrDownloadStage.INSTALLING
        },
        downloadedBytes = downloadedBytes.toLongSaturated(),
        totalBytes = totalBytes.toLongSaturated(),
    )

private fun Int.toBindingThreads(): UInt {
    require(this in 1..8) { "Model thread count must be between 1 and 8." }
    return toUInt()
}

private fun ULong.toLongSaturated(): Long =
    coerceAtMost(Long.MAX_VALUE.toULong()).toLong()
