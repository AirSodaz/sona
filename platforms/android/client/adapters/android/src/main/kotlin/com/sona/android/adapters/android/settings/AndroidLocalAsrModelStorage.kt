package com.sona.android.adapters.android.settings

import android.content.Context
import android.os.StatFs
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDownloadFile
import com.sona.android.application.recording.LocalAsrDownloadProgress
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrDownloadStage
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalAsrModelSource
import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.LocalSherpaModelFiles
import com.sona.android.application.recording.LocalSherpaConfig
import java.io.BufferedInputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URI
import java.net.URL
import java.security.MessageDigest
import java.util.Properties
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import org.apache.commons.compress.compressors.bzip2.BZip2CompressorInputStream
import org.apache.commons.compress.archivers.tar.TarArchiveInputStream

internal class AndroidLocalAsrModelStorage(
    private val appContext: Context,
) {
    val modelsDir = File(appContext.filesDir, "models")

    fun listInstalledModels(): List<InstalledLocalAsrModel> {
        val children = modelsDir.listFiles().orEmpty()
        return children
            .asSequence()
            .filter { it.isDirectory && isManagedInstallName(it.name) }
            .mapNotNull(::readInstalledModel)
            .sortedBy { it.model.displayName.lowercase() }
            .toList()
    }

    suspend fun downloadModel(
        catalogModel: LocalAsrCatalogModel,
        numThreads: Int,
        listener: LocalAsrDownloadProgressListener,
    ): InstalledLocalAsrModel = withContext(Dispatchers.IO) {
        require(catalogModel.id.matches(SAFE_MODEL_ID)) { "Invalid model id." }
        ensureDownloadStorage(catalogModel)
        modelsDir.mkdirs()
        val downloadsDir = File(modelsDir, ".downloads").apply { mkdirs() }
        val staging = File(modelsDir, ".installing-${UUID.randomUUID()}")
        val installed = File(modelsDir, "local-model-${UUID.randomUUID()}")
        check(staging.mkdirs()) { "Unable to prepare local model storage." }
        try {
            installDownloadFile(
                modelId = catalogModel.id,
                spec = catalogModel.download,
                downloadsDir = downloadsDir,
                staging = staging,
                listener = listener,
            )
            catalogModel.vadDownload?.let { vad ->
                installDownloadFile(
                    modelId = catalogModel.id,
                    spec = vad,
                    downloadsDir = downloadsDir,
                    staging = staging,
                    listener = listener,
                )
            }
            catalogModel.punctuationDownload?.let { punctuation ->
                installDownloadFile(
                    modelId = catalogModel.id,
                    spec = punctuation,
                    downloadsDir = downloadsDir,
                    staging = staging,
                    listener = listener,
                )
            }
            listener.onProgress(
                LocalAsrDownloadProgress(catalogModel.id, LocalAsrDownloadStage.VERIFYING),
            )
            val detected = requireSupportedModel(staging)
            require(detected.modelType == catalogModel.modelType) {
                "Downloaded model type did not match the catalog."
            }
            writeManifest(
                staging,
                ModelManifest(
                    catalogModel.id,
                    catalogModel.displayName,
                    LocalAsrModelSource.CATALOG,
                    catalogModel.supportedModes,
                ),
            )
            listener.onProgress(
                LocalAsrDownloadProgress(catalogModel.id, LocalAsrDownloadStage.INSTALLING),
            )
            if (!staging.renameTo(installed)) {
                throw IOException("Unable to install the downloaded local model.")
            }
            detected.rebase(staging, installed).toInstalledModel(
                installRoot = installed,
                id = catalogModel.id,
                displayName = catalogModel.displayName,
                source = LocalAsrModelSource.CATALOG,
                numThreads = numThreads,
                supportedModes = catalogModel.supportedModes,
            )
        } finally {
            staging.deleteRecursively()
        }
    }

    fun validate(modelId: String): Boolean {
        val installed = listInstalledModels().firstOrNull { it.model.id == modelId } ?: return false
        val detected = detectLocalAsrModel(installed.installRoot) ?: return false
        return detected.toInstalledModel(
            installRoot = installed.installRoot,
            id = installed.model.id,
            displayName = installed.model.displayName,
            source = installed.model.source,
            numThreads = installed.model.config.numThreads,
        ).model.let(::localModelIsUsable)
    }

    fun delete(modelId: String) {
        val installed = listInstalledModels().firstOrNull { it.model.id == modelId } ?: return
        deleteManagedInstall(installed.installRoot)
    }

    fun deleteInstall(installed: InstalledLocalAsrModel) {
        deleteManagedInstall(installed.installRoot)
    }

    fun deleteOtherCatalogInstalls(modelId: String, keep: File) {
        listInstalledModels()
            .filter { it.model.id == modelId && it.installRoot != keep }
            .forEach { deleteManagedInstall(it.installRoot) }
    }

    private fun readInstalledModel(root: File): InstalledLocalAsrModel? {
        val detected = detectLocalAsrModel(root) ?: return null
        val manifest = readManifest(root)
        val id = manifest?.id ?: root.canonicalPath
        return detected.toInstalledModel(
            installRoot = root,
            id = id,
            displayName = manifest?.displayName ?: detected.displayName,
            source = manifest?.source ?: LocalAsrModelSource.IMPORTED,
            numThreads = DEFAULT_THREADS,
            supportedModes = manifest?.supportedModes
                ?.takeIf { it.isNotEmpty() }
                ?: detected.supportedModes,
        )
    }

    private fun requireSupportedModel(root: File): DetectedLocalAsrModel {
        val detected = detectLocalAsrModel(root)
            ?: throw IllegalArgumentException(
                "No supported Sherpa-ONNX ASR model was found.",
            )
        if (detected.requiresVad && detected.vadModel == null) {
            throw IllegalArgumentException("This model also requires silero_vad.onnx.")
        }
        return detected
    }

    private suspend fun installDownloadFile(
        modelId: String,
        spec: LocalAsrDownloadFile,
        downloadsDir: File,
        staging: File,
        listener: LocalAsrDownloadProgressListener,
    ) {
        val safeName = spec.fileName.takeIf(::isSafeFileName)
            ?: throw IllegalArgumentException("Invalid catalog file name.")
        val partial = File(downloadsDir, "$modelId-${safeName}.download")
        downloadHttps(spec.url, partial) { downloaded, total ->
            listener.onProgress(
                LocalAsrDownloadProgress(
                    modelId = modelId,
                    stage = LocalAsrDownloadStage.DOWNLOADING,
                    downloadedBytes = downloaded,
                    totalBytes = total,
                ),
            )
        }
        listener.onProgress(LocalAsrDownloadProgress(modelId, LocalAsrDownloadStage.VERIFYING))
        try {
            verifySha256(partial, spec.sha256)
            if (spec.archive) {
                extractTarBz2(partial, staging)
            } else {
                val destination = File(staging, safeName)
                partial.inputStream().buffered().use { input ->
                    destination.outputStream().buffered().use(input::copyTo)
                }
            }
            partial.delete()
        } catch (error: Exception) {
            if (error is HashMismatchException) partial.delete()
            throw error
        }
    }

    private suspend fun downloadHttps(
        url: String,
        destination: File,
        onProgress: (Long, Long) -> Unit,
    ) {
        var currentUrl = requireHttpsUrl(url)
        repeat(MAX_REDIRECTS + 1) { redirectCount ->
            currentCoroutineContext().ensureActive()
            val existingBytes = destination.takeIf(File::isFile)?.length() ?: 0
            val connection = (currentUrl.openConnection() as HttpURLConnection).apply {
                instanceFollowRedirects = false
                connectTimeout = CONNECT_TIMEOUT_MS
                readTimeout = READ_TIMEOUT_MS
                setRequestProperty("User-Agent", "Sona-Android/1.0")
                if (existingBytes > 0) setRequestProperty("Range", "bytes=$existingBytes-")
            }
            try {
                when (val status = connection.responseCode) {
                    in 300..399 -> {
                        require(redirectCount < MAX_REDIRECTS) { "Too many download redirects." }
                        val location = connection.getHeaderField("Location")
                            ?: throw IOException("Download redirect did not include a location.")
                        currentUrl = requireHttpsUrl(URL(currentUrl, location).toString())
                    }
                    HttpURLConnection.HTTP_PARTIAL,
                    HttpURLConnection.HTTP_OK,
                    -> {
                        val append = status == HttpURLConnection.HTTP_PARTIAL && existingBytes > 0
                        val startingBytes = if (append) existingBytes else 0
                        val responseBytes = connection.getHeaderField("Content-Length")
                            ?.toLongOrNull()
                            ?.coerceAtLeast(0)
                            ?: 0
                        val totalBytes = startingBytes + responseBytes
                        connection.inputStream.buffered().use { input ->
                            FileOutputStream(destination, append).buffered().use { output ->
                                val buffer = ByteArray(DOWNLOAD_BUFFER_SIZE)
                                var downloaded = startingBytes
                                while (true) {
                                    currentCoroutineContext().ensureActive()
                                    val read = input.read(buffer)
                                    if (read < 0) break
                                    output.write(buffer, 0, read)
                                    downloaded += read
                                    onProgress(downloaded, totalBytes)
                                }
                            }
                        }
                        return
                    }
                    HTTP_RANGE_NOT_SATISFIABLE -> {
                        destination.delete()
                        if (redirectCount == MAX_REDIRECTS) {
                            throw IOException("Unable to restart model download.")
                        }
                    }
                    else -> throw IOException("Model download failed with HTTP $status.")
                }
            } finally {
                connection.disconnect()
            }
        }
        throw IOException("Model download did not complete.")
    }

    private fun extractTarBz2(archive: File, target: File) {
        var entryCount = 0
        var extractedBytes = 0L
        TarArchiveInputStream(
            BZip2CompressorInputStream(BufferedInputStream(FileInputStream(archive))),
        ).use { tar ->
            while (true) {
                val entry = tar.nextEntry ?: break
                entryCount += 1
                require(entryCount <= MAX_ARCHIVE_ENTRIES) { "Model archive has too many entries." }
                require(!entry.isSymbolicLink && !entry.isLink && !entry.isCharacterDevice &&
                    !entry.isBlockDevice && !entry.isFIFO) {
                    "Model archive contains an unsupported entry."
                }
                val destination = safeArchiveDestination(target, entry.name)
                if (entry.isDirectory) {
                    check(destination.mkdirs() || destination.isDirectory) {
                        "Unable to create an extracted model directory."
                    }
                } else if (entry.isFile) {
                    require(entry.size in 0..MAX_ARCHIVE_FILE_BYTES) {
                        "Model archive contains an oversized file."
                    }
                    extractedBytes += entry.size
                    require(extractedBytes <= MAX_ARCHIVE_TOTAL_BYTES) {
                        "Model archive is too large."
                    }
                    destination.parentFile?.let { parent ->
                        check(parent.mkdirs() || parent.isDirectory) {
                            "Unable to create an extracted model directory."
                        }
                    }
                    FileOutputStream(destination).buffered().use { output -> tar.copyTo(output) }
                }
            }
        }
    }

    private fun ensureDownloadStorage(model: LocalAsrCatalogModel) {
        if (model.estimatedSizeBytes <= 0) return
        val available = StatFs(appContext.filesDir.absolutePath).availableBytes
        val required = model.estimatedSizeBytes * 2 + DOWNLOAD_STORAGE_MARGIN_BYTES
        require(available >= required) { "Not enough storage for this model." }
    }

    private fun deleteManagedInstall(root: File) {
        val canonicalModelsDir = modelsDir.canonicalFile
        val canonicalRoot = root.canonicalFile
        require(canonicalRoot.parentFile == canonicalModelsDir && isManagedInstallName(root.name)) {
            "Refusing to delete an unmanaged model path."
        }
        if (!canonicalRoot.deleteRecursively() && canonicalRoot.exists()) {
            throw IOException("Unable to delete the local model.")
        }
    }

    companion object {
        private const val DEFAULT_THREADS = 2
        private const val MAX_REDIRECTS = 5
        private const val CONNECT_TIMEOUT_MS = 20_000
        private const val READ_TIMEOUT_MS = 60_000
        private const val DOWNLOAD_BUFFER_SIZE = 64 * 1_024
        private const val HTTP_RANGE_NOT_SATISFIABLE = 416
        private const val MAX_ARCHIVE_ENTRIES = 20_000
        private const val MAX_ARCHIVE_FILE_BYTES = 4L * 1_024 * 1_024 * 1_024
        private const val MAX_ARCHIVE_TOTAL_BYTES = 8L * 1_024 * 1_024 * 1_024
        private const val DOWNLOAD_STORAGE_MARGIN_BYTES = 128L * 1_024 * 1_024
        private val SAFE_MODEL_ID = Regex("[A-Za-z0-9._-]+")
    }
}

internal data class InstalledLocalAsrModel(
    val installRoot: File,
    val model: LocalAsrModel,
)

internal data class DetectedLocalAsrModel(
    val displayName: String,
    val modelPath: File,
    val modelType: String,
    val files: LocalSherpaModelFiles,
    val vadModel: File?,
    val punctuationModel: File?,
    val requiresVad: Boolean,
    val supportedModes: Set<AsrMode>,
) {
    fun rebase(oldRoot: File, newRoot: File): DetectedLocalAsrModel = copy(
        modelPath = File(newRoot, modelPath.relativeTo(oldRoot).path),
        vadModel = vadModel?.let { File(newRoot, it.relativeTo(oldRoot).path) },
        punctuationModel = punctuationModel?.let { File(newRoot, it.relativeTo(oldRoot).path) },
    )

    fun toInstalledModel(
        installRoot: File,
        id: String,
        displayName: String,
        source: LocalAsrModelSource,
        numThreads: Int,
        supportedModes: Set<AsrMode> = this.supportedModes,
    ): InstalledLocalAsrModel = InstalledLocalAsrModel(
        installRoot = installRoot,
        model = LocalAsrModel(
            id = id,
            displayName = displayName,
            config = LocalSherpaConfig(
                modelPath = modelPath.absolutePath,
                numThreads = numThreads.coerceIn(1, 8),
                modelType = modelType,
                punctuationModel = punctuationModel?.absolutePath,
                vadModel = vadModel?.absolutePath,
                fileConfig = files,
            ),
            supportedModes = supportedModes,
            sizeBytes = installRoot.walkTopDown().filter(File::isFile).sumOf(File::length),
            source = source,
        ),
    )
}

internal fun detectLocalAsrModel(root: File): DetectedLocalAsrModel? {
    val vad = root.walkTopDown().firstOrNull { it.isFile && it.name == "silero_vad.onnx" }
    val punctuation = root.walkTopDown().firstOrNull {
        it.isFile && it.extension == "onnx" &&
            it.parentFile?.name?.contains("punct", ignoreCase = true) == true
    }
    return root.walkTopDown().filter(File::isDirectory).mapNotNull { directory ->
        val files = directory.listFiles()?.filter(File::isFile)?.associateBy { it.name }
            ?: return@mapNotNull null
        val tokens = files.values.firstOrNull { it.name.endsWith("tokens.txt") }
        val encoder = files["encoder.int8.onnx"] ?: files["encoder.onnx"]
        val decoder = files["decoder.int8.onnx"] ?: files["decoder.onnx"]
        val joiner = files["joiner.int8.onnx"] ?: files["joiner.onnx"]
        val model = files["model.int8.onnx"] ?: files["model.onnx"]
        val encoderAdaptor = files["encoder_adaptor.int8.onnx"] ?: files["encoder_adaptor.onnx"]
        val llm = files["llm.int8.onnx"] ?: files["llm.fp16.onnx"] ?: files["llm.fp32.onnx"]
        val embedding = files["embedding.int8.onnx"] ?: files["embedding.onnx"]
        val tokenizerDirectory = directory.listFiles()?.firstOrNull {
            it.isDirectory && (it.name.contains("tokenizer", true) || it.name.startsWith("Qwen"))
        }
        when {
            encoderAdaptor != null && llm != null && embedding != null &&
                tokenizerDirectory != null -> DetectedLocalAsrModel(
                directory.name, directory, "funasr-nano",
                LocalSherpaModelFiles(
                    encoderAdaptor = encoderAdaptor.name,
                    llm = llm.name,
                    embedding = embedding.name,
                    tokenizer = tokenizerDirectory.name,
                    tokens = tokens?.name,
                ),
                vad, punctuation, true, setOf(AsrMode.BATCH),
            )
            files["conv_frontend.onnx"] != null && encoder != null && decoder != null &&
                File(directory, "tokenizer").isDirectory -> DetectedLocalAsrModel(
                directory.name, directory, "qwen3-asr",
                LocalSherpaModelFiles(
                    convFrontend = "conv_frontend.onnx",
                    encoder = encoder.name,
                    decoder = decoder.name,
                    tokenizer = "tokenizer",
                ),
                vad, punctuation, true, setOf(AsrMode.BATCH),
            )
            encoder != null && decoder != null && tokens != null &&
                files.keys.any { it.contains("whisper", true) || it.contains("turbo", true) ||
                    it.contains("large-v3", true) || it.contains("medium-aishell", true) } ->
                DetectedLocalAsrModel(
                    directory.name, directory, "whisper",
                    LocalSherpaModelFiles(
                        encoder = encoder.name,
                        decoder = decoder.name,
                        tokens = tokens.name,
                    ),
                    vad, punctuation, true, setOf(AsrMode.BATCH),
                )
            encoder != null && decoder != null && tokens != null &&
                directory.name.contains("fire-red", true) -> DetectedLocalAsrModel(
                directory.name, directory, "fire-red-asr",
                LocalSherpaModelFiles(
                    encoder = encoder.name,
                    decoder = decoder.name,
                    tokens = tokens.name,
                ),
                vad, punctuation, true, setOf(AsrMode.BATCH),
            )
            encoder != null && decoder != null && joiner != null && tokens != null -> DetectedLocalAsrModel(
                directory.name, directory, "zipformer",
                LocalSherpaModelFiles(
                    encoder = encoder.name,
                    decoder = decoder.name,
                    joiner = joiner.name,
                    tokens = tokens.name,
                ),
                vad, punctuation, false, setOf(AsrMode.STREAMING),
            )
            encoder != null && decoder != null && tokens != null -> DetectedLocalAsrModel(
                directory.name, directory, "paraformer",
                LocalSherpaModelFiles(
                    encoder = encoder.name,
                    decoder = decoder.name,
                    tokens = tokens.name,
                ),
                vad, punctuation, false, setOf(AsrMode.STREAMING),
            )
            model != null && tokens != null -> {
                val type = if (directory.name.contains("dolphin", ignoreCase = true)) {
                    "dolphin"
                } else {
                    "sensevoice"
                }
                DetectedLocalAsrModel(
                    directory.name, directory, type,
                    LocalSherpaModelFiles(model = model.name, tokens = tokens.name),
                    vad,
                    punctuation,
                    true,
                    if (type == "sensevoice" || type == "dolphin") {
                        setOf(AsrMode.STREAMING, AsrMode.BATCH)
                    } else {
                        setOf(AsrMode.BATCH)
                    },
                )
            }
            else -> null
        }
    }.firstOrNull()
}

internal fun localModelIsUsable(model: LocalAsrModel): Boolean {
    val config = model.config
    val directory = File(config.modelPath)
    val files = config.fileConfig ?: return false
    if (!directory.isDirectory || config.modelType.isBlank()) return false
    fun exists(name: String?): Boolean = name != null && File(directory, name).let {
        (it.isFile && it.length() > 0) || (it.isDirectory && it.list().orEmpty().isNotEmpty())
    }
    if (config.modelType !in TOKEN_OPTIONAL_MODEL_TYPES && !exists(files.tokens)) return false
    val primaryFilesExist = when (config.modelType) {
        "zipformer" -> exists(files.encoder) && exists(files.decoder) && exists(files.joiner)
        "paraformer" -> exists(files.encoder) && exists(files.decoder)
        "whisper", "fire-red-asr" -> exists(files.encoder) && exists(files.decoder)
        "qwen3-asr" -> exists(files.convFrontend) && exists(files.encoder) &&
            exists(files.decoder) && exists(files.tokenizer)
        "funasr-nano" -> exists(files.encoderAdaptor) && exists(files.llm) &&
            exists(files.embedding) && exists(files.tokenizer)
        else -> exists(files.model)
    }
    val vadExistsWhenRequired = config.modelType !in OFFLINE_STREAMING_MODEL_TYPES ||
        config.vadModel?.let { File(it).isFile && File(it).length() > 0 } == true
    return primaryFilesExist && vadExistsWhenRequired
}

private data class ModelManifest(
    val id: String,
    val displayName: String,
    val source: LocalAsrModelSource,
    val supportedModes: Set<AsrMode>,
)

private fun writeManifest(root: File, manifest: ModelManifest) {
    val properties = Properties().apply {
        setProperty("id", manifest.id)
        setProperty("displayName", manifest.displayName)
        setProperty("source", manifest.source.name)
        setProperty("modes", manifest.supportedModes.joinToString(",", transform = AsrMode::name))
    }
    File(root, MODEL_MANIFEST).outputStream().buffered().use { properties.store(it, null) }
}

private fun readManifest(root: File): ModelManifest? = runCatching {
    val properties = Properties().apply {
        File(root, MODEL_MANIFEST).inputStream().buffered().use(::load)
    }
    ModelManifest(
        id = properties.getProperty("id").takeIf(String::isNotBlank) ?: return null,
        displayName = properties.getProperty("displayName").takeIf(String::isNotBlank) ?: return null,
        source = runCatching {
            LocalAsrModelSource.valueOf(properties.getProperty("source"))
        }.getOrDefault(LocalAsrModelSource.IMPORTED),
        supportedModes = properties.getProperty("modes")
            ?.split(',')
            ?.mapNotNullTo(mutableSetOf()) { runCatching { AsrMode.valueOf(it) }.getOrNull() }
            .orEmpty(),
    )
}.getOrNull()

internal fun verifySha256(file: File, expected: String?) {
    if (expected.isNullOrBlank()) return
    val digest = MessageDigest.getInstance("SHA-256")
    file.inputStream().buffered().use { input ->
        val buffer = ByteArray(64 * 1_024)
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            digest.update(buffer, 0, read)
        }
    }
    val actual = digest.digest().joinToString("") { "%02x".format(it) }
    if (!actual.equals(expected, ignoreCase = true)) throw HashMismatchException()
}

private class HashMismatchException : IOException("Downloaded model checksum did not match.")

internal fun safeArchiveDestination(root: File, entryName: String): File {
    require(
        entryName.isNotBlank() &&
            !entryName.startsWith('/') &&
            !entryName.contains('\\') &&
            entryName.split('/').none { it == "." || it == ".." },
    ) { "Invalid archive path." }
    val normalized = URI(null, null, "/$entryName", null).normalize().path.removePrefix("/")
    require(normalized.isNotBlank() && normalized != ".." && !normalized.startsWith("../")) {
        "Model archive contains an unsafe path."
    }
    val destination = File(root, normalized).canonicalFile
    val canonicalRoot = root.canonicalFile
    require(destination.path.startsWith(canonicalRoot.path + File.separator)) {
        "Model archive contains an unsafe path."
    }
    return destination
}

private fun requireHttpsUrl(value: String): URL {
    val url = URL(value)
    require(url.protocol.equals("https", ignoreCase = true) && url.host.isNotBlank()) {
        "Model downloads require HTTPS."
    }
    return url
}

private fun isSafeFileName(name: String): Boolean =
    name.isNotBlank() && name != "." && name != ".." && name == File(name).name

private fun isManagedInstallName(name: String): Boolean =
    name.startsWith("local-model-") || name.startsWith("local-streaming-")

private const val MODEL_MANIFEST = ".sona-model.properties"
private val OFFLINE_STREAMING_MODEL_TYPES = setOf(
    "sensevoice",
    "whisper",
    "funasr-nano",
    "fire-red-asr",
    "dolphin",
    "qwen3-asr",
)
private val TOKEN_OPTIONAL_MODEL_TYPES = setOf("qwen3-asr", "funasr-nano")
