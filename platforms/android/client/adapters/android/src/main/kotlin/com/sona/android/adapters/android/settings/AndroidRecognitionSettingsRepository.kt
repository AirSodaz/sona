package com.sona.android.adapters.android.settings

import android.content.Context
import android.net.Uri
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import androidx.documentfile.provider.DocumentFile
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalSherpaModelFiles
import com.sona.android.application.recording.LocalSherpaStreamingConfig
import com.sona.android.application.recording.RecognitionEngine
import com.sona.android.application.recording.RecognitionSettings
import com.sona.android.application.recording.RecognitionSettingsPort
import java.io.File
import java.io.IOException
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

class AndroidRecognitionSettingsRepository internal constructor(
    private val appContext: Context,
    private val dataStore: DataStore<Preferences>,
) : RecognitionSettingsPort {
    private val importMutex = Mutex()

    override val settings: Flow<RecognitionSettings> = dataStore.data
        .catch { error ->
            if (error is IOException) emit(emptyPreferences()) else throw error
        }
        .map(::toSettings)
        .distinctUntilChanged()

    override suspend fun load(): RecognitionSettings = settings.first()

    override suspend fun selectEngine(engine: RecognitionEngine) {
        dataStore.edit { it[ENGINE] = engine.name }
    }

    override suspend fun importLocalModel(sourceLocation: String): LocalAsrModel =
        withContext(Dispatchers.IO) {
            importMutex.withLock {
                require(sourceLocation.isNotBlank()) { "Model folder is required." }
                val source = DocumentFile.fromTreeUri(appContext, Uri.parse(sourceLocation))
                    ?.takeIf { it.isDirectory }
                    ?: throw IllegalArgumentException("The selected model folder is unavailable.")
                val modelsDir = File(appContext.filesDir, "models")
                val staging = File(modelsDir, ".importing")
                val installed = File(modelsDir, "local-streaming-${UUID.randomUUID()}")
                val previousModelPath = load().localModel?.config?.modelPath
                modelsDir.mkdirs()
                staging.deleteRecursively()
                check(staging.mkdirs()) { "Unable to prepare local model storage." }
                try {
                    copyTree(source, staging)
                    val detected = detectLocalAsrModel(staging)
                        ?: throw IllegalArgumentException(
                            "No supported Sherpa-ONNX streaming model was found in that folder.",
                        )
                    if (detected.requiresVad && detected.vadModel == null) {
                        throw IllegalArgumentException(
                            "This offline model also requires silero_vad.onnx in the selected folder.",
                        )
                    }
                    if (!staging.renameTo(installed)) {
                        throw IOException("Unable to install the selected local model.")
                    }

                    val model = detected.rebase(staging, installed).toApplicationModel()
                    try {
                        persist(model)
                    } catch (error: Exception) {
                        installed.deleteRecursively()
                        throw error
                    }
                    deletePreviousManagedModel(previousModelPath, modelsDir, installed)
                    model
                } finally {
                    staging.deleteRecursively()
                }
            }
        }

    private suspend fun persist(model: LocalAsrModel) {
        val config = model.config
        val files = config.fileConfig ?: LocalSherpaModelFiles()
        dataStore.edit { preferences ->
            preferences[ENGINE] = RecognitionEngine.LOCAL.name
            preferences[MODEL_NAME] = model.displayName
            preferences[MODEL_PATH] = config.modelPath
            preferences[MODEL_TYPE] = config.modelType
            preferences[MODEL_THREADS] = config.numThreads
            putOptional(preferences, VAD_PATH, config.vadModel)
            putOptional(preferences, FILE_ENCODER, files.encoder)
            putOptional(preferences, FILE_DECODER, files.decoder)
            putOptional(preferences, FILE_MODEL, files.model)
            putOptional(preferences, FILE_JOINER, files.joiner)
            putOptional(preferences, FILE_TOKENS, files.tokens)
        }
    }

    private fun toSettings(preferences: Preferences): RecognitionSettings {
        val engine = runCatching {
            RecognitionEngine.valueOf(preferences[ENGINE].orEmpty())
        }.getOrDefault(RecognitionEngine.ONLINE)
        val modelPath = preferences[MODEL_PATH]
        val model = modelPath?.takeIf { File(it).isDirectory }?.let { path ->
            LocalAsrModel(
                displayName = preferences[MODEL_NAME].orEmpty().ifBlank { File(path).name },
                config = LocalSherpaStreamingConfig(
                    modelPath = path,
                    numThreads = (preferences[MODEL_THREADS] ?: DEFAULT_THREADS).coerceIn(1, 8),
                    modelType = preferences[MODEL_TYPE].orEmpty(),
                    vadModel = preferences[VAD_PATH]?.takeIf { File(it).isFile },
                    fileConfig = LocalSherpaModelFiles(
                        encoder = preferences[FILE_ENCODER],
                        decoder = preferences[FILE_DECODER],
                        model = preferences[FILE_MODEL],
                        joiner = preferences[FILE_JOINER],
                        tokens = preferences[FILE_TOKENS],
                    ),
                ),
            ).takeIf(::localModelIsUsable)
        }
        return RecognitionSettings(engine = engine, localModel = model)
    }

    private fun copyTree(source: DocumentFile, target: File, depth: Int = 0) {
        require(depth <= MAX_TREE_DEPTH) { "The selected model folder is too deeply nested." }
        source.listFiles().forEach { child ->
            val name = child.name?.takeIf(::isSafeFileName)
                ?: throw IllegalArgumentException("The model folder contains an invalid file name.")
            val destination = File(target, name)
            if (child.isDirectory) {
                check(destination.mkdir()) { "Unable to create a local model directory." }
                copyTree(child, destination, depth + 1)
            } else if (child.isFile) {
                val input = appContext.contentResolver.openInputStream(child.uri)
                    ?: throw IOException("Unable to read $name.")
                input.use { sourceStream ->
                    destination.outputStream().buffered().use(sourceStream::copyTo)
                }
            }
        }
    }

    companion object {
        private const val DATASTORE_NAME = "recognition_settings"
        private const val DEFAULT_THREADS = 2
        private const val MAX_TREE_DEPTH = 8
        private val ENGINE = stringPreferencesKey("engine")
        private val MODEL_NAME = stringPreferencesKey("local_model_name")
        private val MODEL_PATH = stringPreferencesKey("local_model_path")
        private val MODEL_TYPE = stringPreferencesKey("local_model_type")
        private val MODEL_THREADS = intPreferencesKey("local_model_threads")
        private val VAD_PATH = stringPreferencesKey("local_vad_path")
        private val FILE_ENCODER = stringPreferencesKey("local_file_encoder")
        private val FILE_DECODER = stringPreferencesKey("local_file_decoder")
        private val FILE_MODEL = stringPreferencesKey("local_file_model")
        private val FILE_JOINER = stringPreferencesKey("local_file_joiner")
        private val FILE_TOKENS = stringPreferencesKey("local_file_tokens")

        fun create(context: Context): AndroidRecognitionSettingsRepository {
            val appContext = context.applicationContext
            return AndroidRecognitionSettingsRepository(
                appContext = appContext,
                dataStore = androidx.datastore.preferences.core.PreferenceDataStoreFactory.create(
                    corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    produceFile = {
                        appContext.preferencesDataStoreFile(DATASTORE_NAME)
                    },
                ),
            )
        }
    }
}

private fun deletePreviousManagedModel(
    previousModelPath: String?,
    modelsDir: File,
    installed: File,
) {
    val previous = previousModelPath?.let(::File)?.let { path ->
        runCatching { path.canonicalFile }.getOrNull()
    } ?: return
    val canonicalModelsDir = runCatching { modelsDir.canonicalFile }.getOrNull() ?: return
    val managedInstall = generateSequence(previous) { it.parentFile }
        .takeWhile { it != canonicalModelsDir }
        .firstOrNull {
            it.parentFile == canonicalModelsDir && it.name.startsWith("local-streaming-")
        }
    val canonicalInstalled = runCatching { installed.canonicalFile }.getOrNull() ?: return
    if (managedInstall != null && managedInstall != canonicalInstalled) {
        runCatching { managedInstall.deleteRecursively() }
    }
}

private fun localModelIsUsable(model: LocalAsrModel): Boolean {
    val config = model.config
    val directory = File(config.modelPath)
    val files = config.fileConfig ?: return false
    if (!directory.isDirectory || config.modelType.isBlank()) return false
    fun exists(name: String?): Boolean = name != null && File(directory, name).isFile
    if (!exists(files.tokens)) return false
    val primaryFilesExist = when (config.modelType) {
        "zipformer" -> exists(files.encoder) && exists(files.decoder) && exists(files.joiner)
        "paraformer" -> exists(files.encoder) && exists(files.decoder)
        else -> exists(files.model)
    }
    val vadExistsWhenRequired = config.modelType !in OFFLINE_STREAMING_MODEL_TYPES ||
        config.vadModel?.let { File(it).isFile } == true
    return primaryFilesExist && vadExistsWhenRequired
}

internal data class DetectedLocalAsrModel(
    val displayName: String,
    val modelPath: File,
    val modelType: String,
    val files: LocalSherpaModelFiles,
    val vadModel: File?,
    val requiresVad: Boolean,
) {
    fun rebase(oldRoot: File, newRoot: File): DetectedLocalAsrModel = copy(
        modelPath = File(newRoot, modelPath.relativeTo(oldRoot).path),
        vadModel = vadModel?.let { File(newRoot, it.relativeTo(oldRoot).path) },
    )

    fun toApplicationModel(): LocalAsrModel = LocalAsrModel(
        displayName = displayName,
        config = LocalSherpaStreamingConfig(
            modelPath = modelPath.absolutePath,
            numThreads = 2,
            modelType = modelType,
            vadModel = vadModel?.absolutePath,
            fileConfig = files,
        ),
    )
}

internal fun detectLocalAsrModel(root: File): DetectedLocalAsrModel? {
    val vad = root.walkTopDown().firstOrNull { it.isFile && it.name == "silero_vad.onnx" }
    return root.walkTopDown().filter(File::isDirectory).mapNotNull { directory ->
        val files = directory.listFiles()?.filter(File::isFile)?.associateBy { it.name }
            ?: return@mapNotNull null
        val tokens = files["tokens.txt"] ?: return@mapNotNull null
        val encoder = files["encoder.int8.onnx"] ?: files["encoder.onnx"]
        val decoder = files["decoder.int8.onnx"] ?: files["decoder.onnx"]
        val joiner = files["joiner.int8.onnx"] ?: files["joiner.onnx"]
        val model = files["model.int8.onnx"] ?: files["model.onnx"]
        when {
            encoder != null && decoder != null && joiner != null -> DetectedLocalAsrModel(
                directory.name, directory, "zipformer",
                LocalSherpaModelFiles(
                    encoder = encoder.name,
                    decoder = decoder.name,
                    joiner = joiner.name,
                    tokens = tokens.name,
                ),
                vad, false,
            )
            encoder != null && decoder != null -> DetectedLocalAsrModel(
                directory.name, directory, "paraformer",
                LocalSherpaModelFiles(
                    encoder = encoder.name,
                    decoder = decoder.name,
                    tokens = tokens.name,
                ),
                vad, false,
            )
            model != null -> {
                val type = if (directory.name.contains("dolphin", ignoreCase = true)) {
                    "dolphin"
                } else {
                    "sensevoice"
                }
                DetectedLocalAsrModel(
                    directory.name, directory, type,
                    LocalSherpaModelFiles(model = model.name, tokens = tokens.name),
                    vad, true,
                )
            }
            else -> null
        }
    }.firstOrNull()
}

private fun isSafeFileName(name: String): Boolean =
    name.isNotBlank() && name != "." && name != ".." && name == File(name).name

private fun putOptional(
    preferences: androidx.datastore.preferences.core.MutablePreferences,
    key: Preferences.Key<String>,
    value: String?,
) {
    if (value == null) preferences.remove(key) else preferences[key] = value
}

private val OFFLINE_STREAMING_MODEL_TYPES = setOf(
    "sensevoice",
    "whisper",
    "funasr-nano",
    "fire-red-asr",
    "dolphin",
    "qwen3-asr",
)
