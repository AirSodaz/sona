package com.sona.android.adapters.android.audio

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.content.pm.ServiceInfo
import android.provider.OpenableColumns
import androidx.core.app.NotificationCompat
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.Data
import androidx.work.ExistingWorkPolicy
import androidx.work.ForegroundInfo
import androidx.work.ListenableWorker
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.WorkerFactory
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import com.sona.android.application.recording.AudioImportEngine
import com.sona.android.application.recording.AudioImportFailure
import com.sona.android.application.recording.AudioImportJob
import com.sona.android.application.recording.AudioImportJobPort
import com.sona.android.application.recording.AudioImportJobState
import com.sona.android.application.recording.AudioImportProgressListener
import com.sona.android.application.recording.AudioImportSource
import com.sona.android.application.recording.AudioImportStage
import com.sona.android.application.recording.AudioImportTarget
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.RunAudioImport
import com.sona.android.application.recording.RunAudioImportOutcome
import com.sona.android.application.recovery.RecoveryItemInput
import com.sona.android.application.recovery.RecoveryStage
import com.sona.android.adapters.android.recovery.RecoveryCoordinator
import com.sona.android.adapters.android.data.isFileWithinRoot
import java.io.File
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

class AndroidAudioImportJobAdapter private constructor(
    private val context: Context,
    private val workManager: Lazy<WorkManager>,
    private val recovery: RecoveryCoordinator,
) : AudioImportJobPort {
    companion object {
        fun create(context: Context, recovery: RecoveryCoordinator): AndroidAudioImportJobAdapter = AndroidAudioImportJobAdapter(
            context = context.applicationContext,
            workManager = lazy { WorkManager.getInstance(context.applicationContext) },
            recovery = recovery,
        )
    }

    override val state: Flow<AudioImportJobState> = flow {
        emitAll(
            workManager.value.getWorkInfosForUniqueWorkFlow(UNIQUE_WORK_NAME).map { workInfos ->
                workInfos.relevantAudioImport()?.toApplicationState() ?: AudioImportJobState.Idle
            },
        )
    }

    override suspend fun enqueue(job: AudioImportJob) {
        var persistedJob = job
        val displayName = when (val target = job.target) {
            is AudioImportTarget.NewImport -> {
                val uri = Uri.parse(target.source.locator)
                if (uri.scheme == "content") {
                    context.contentResolver.takePersistableUriPermission(
                        uri,
                        Intent.FLAG_GRANT_READ_URI_PERMISSION,
                    )
                    val name = displayName(uri) ?: "Imported audio"
                    val staged = stageRecoverySource(job.id, uri, name)
                    persistedJob = job.copy(
                        target = AudioImportTarget.NewImport(AudioImportSource(staged.absolutePath)),
                    )
                    name
                } else {
                    File(target.source.locator).name.takeIf(String::isNotBlank)
                }
            }
            is AudioImportTarget.ExistingRecording -> target.displayName
        }
        recovery.upsert(persistedJob.toRecoveryInput(displayName, RecoveryStage.QUEUED, 0.0))
        val constraints = Constraints.Builder().apply {
            if (persistedJob.engine is AudioImportEngine.Online) {
                setRequiredNetworkType(NetworkType.CONNECTED)
            }
        }.build()
        val request = OneTimeWorkRequestBuilder<AudioImportWorker>()
            .setInputData(persistedJob.toWorkData(displayName))
            .setConstraints(constraints)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .addTag(persistedJob.id)
            .addTag("$ENQUEUED_AT_TAG_PREFIX${System.currentTimeMillis()}")
            .build()
        workManager.value.enqueueUniqueWork(
            UNIQUE_WORK_NAME,
            ExistingWorkPolicy.APPEND_OR_REPLACE,
            request,
        )
    }

    override suspend fun cancel(jobId: String) {
        workManager.value.cancelAllWorkByTag(jobId)
        recovery.resolve(jobId).item?.filePath?.let { deleteRecoverySource(context, it) }
    }

    private fun displayName(uri: Uri): String? = context.contentResolver.query(
        uri,
        arrayOf(OpenableColumns.DISPLAY_NAME),
        null,
        null,
        null,
    )?.use { cursor ->
        val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        if (index >= 0 && cursor.moveToFirst() && !cursor.isNull(index)) cursor.getString(index) else null
    }

    private suspend fun stageRecoverySource(jobId: String, uri: Uri, displayName: String): File =
        withContext(Dispatchers.IO) {
            require(jobId.matches(Regex("[A-Za-z0-9-]{1,64}"))) { "Audio import job ID is invalid." }
            val root = File(context.filesDir, "recovery/import-sources/$jobId")
            root.mkdirs()
            val extension = displayName.substringAfterLast('.', "")
                .takeIf { it.matches(Regex("[A-Za-z0-9]{1,8}")) }
                ?.let { ".$it" }
                .orEmpty()
            val destination = File(root, "source$extension")
            val partial = File(root, "source$extension.partial")
            try {
                context.contentResolver.openInputStream(uri)?.use { input ->
                    partial.outputStream().use(input::copyTo)
                } ?: throw IllegalArgumentException("Unable to open audio source.")
                require(partial.length() > 0) { "Audio source is empty." }
                if (!partial.renameTo(destination)) {
                    partial.copyTo(destination, overwrite = true)
                    partial.delete()
                }
                destination
            } catch (error: Throwable) {
                partial.delete()
                root.deleteRecursively()
                throw error
            }
        }
}

class AudioImportWorkerFactory(
    private val runAudioImport: RunAudioImport,
    private val recovery: RecoveryCoordinator,
) : WorkerFactory() {
    override fun createWorker(
        appContext: Context,
        workerClassName: String,
        workerParameters: WorkerParameters,
    ): ListenableWorker? = if (workerClassName == AudioImportWorker::class.java.name) {
        AudioImportWorker(appContext, workerParameters, runAudioImport, recovery)
    } else {
        null
    }
}

class AudioImportWorker internal constructor(
    appContext: Context,
    params: WorkerParameters,
    private val runAudioImport: RunAudioImport,
    private val recovery: RecoveryCoordinator,
) : CoroutineWorker(appContext, params) {
    override suspend fun doWork(): Result {
        val job = inputData.toAudioImportJob()
            ?: return Result.failure(failureData(AudioImportFailure.INVALID_SOURCE, null))
        setForeground(foregroundInfo(AudioImportStage.STAGING, null))
        val displayName = inputData.getString(KEY_DISPLAY_NAME)
        val progress = AudioImportProgressListener { stage, percent ->
            setProgress(progressData(job, displayName, stage, percent))
            setForeground(foregroundInfo(stage, percent))
            recovery.upsert(job.toRecoveryInput(displayName, stage.toRecoveryStage(), (percent ?: 0) / 100.0))
        }
        return when (
            val outcome = runAudioImport(
                job = job,
                progress = progress,
                allowTranscriptionWarning = runAttemptCount >= MAX_RETRY_ATTEMPTS - 1,
            )
        ) {
            is RunAudioImportOutcome.Completed -> {
                recovery.resolve(job.id).item?.filePath?.let {
                    deleteRecoverySource(applicationContext, it)
                }
                Result.success(workDataOf(
                    KEY_JOB_ID to job.id,
                    KEY_HISTORY_ID to outcome.historyId,
                    KEY_TRANSCRIPTION_WARNING to outcome.transcriptionWarning,
                ))
            }
            is RunAudioImportOutcome.RetryableFailure -> {
                if (runAttemptCount < MAX_RETRY_ATTEMPTS - 1) {
                    Result.retry()
                } else {
                    Result.failure(failureData(outcome.reason, job.id))
                }
            }
            is RunAudioImportOutcome.TerminalFailure ->
                Result.failure(failureData(outcome.reason, job.id))
        }
    }

    override suspend fun getForegroundInfo(): ForegroundInfo =
        foregroundInfo(AudioImportStage.QUEUED, null)

    private fun foregroundInfo(stage: AudioImportStage, percent: Int?): ForegroundInfo {
        createNotificationChannel()
        val launchIntent = applicationContext.packageManager
            .getLaunchIntentForPackage(applicationContext.packageName)
        val pendingIntent = launchIntent?.let {
            PendingIntent.getActivity(
                applicationContext,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        val title = applicationContext.applicationInfo.loadLabel(
            applicationContext.packageManager,
        ).toString()
        val notification = NotificationCompat.Builder(applicationContext, NOTIFICATION_CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle(title)
            .setContentText(stage.notificationText())
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(pendingIntent)
            .apply {
                if (percent == null) setProgress(0, 0, true) else setProgress(100, percent, false)
            }
            .build()
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            ForegroundInfo(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
        } else {
            ForegroundInfo(NOTIFICATION_ID, notification)
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = applicationContext.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                NOTIFICATION_CHANNEL_ID,
                "Audio imports",
                NotificationManager.IMPORTANCE_LOW,
            ),
        )
    }
}

internal fun AudioImportJob.toRecoveryInput(
    displayName: String?,
    stage: RecoveryStage = RecoveryStage.QUEUED,
    progress: Double = 0.0,
) = RecoveryItemInput(
    id = id,
    filename = displayName.orEmpty().ifBlank { "Imported audio" },
    filePath = when (val target = target) {
        is AudioImportTarget.NewImport -> target.source.locator
        is AudioImportTarget.ExistingRecording -> target.audioPath
    },
    historyId = (target as? AudioImportTarget.ExistingRecording)?.historyId,
    historyTitle = displayName,
    stage = stage,
    progress = progress.coerceIn(0.0, 1.0),
    payload = toRecoveryPayload(),
)

internal fun AudioImportJob.toRecoveryPayload(): String = buildJsonObject {
    put("androidAudioImportV1", buildJsonObject {
        put("id", this@toRecoveryPayload.id)
        when (val value = this@toRecoveryPayload.target) {
            is AudioImportTarget.NewImport -> {
                put("target", "new")
                put("source", value.source.locator)
            }
            is AudioImportTarget.ExistingRecording -> {
                put("target", "existing")
                put("historyId", value.historyId)
                put("audioPath", value.audioPath)
                put("displayName", value.displayName)
                put("durationMillis", value.durationMillis)
            }
        }
        when (val value = this@toRecoveryPayload.engine) {
            is AudioImportEngine.Local -> {
                put("engine", "local")
                put("modelId", value.modelId)
            }
            is AudioImportEngine.Online -> {
                put("engine", "online")
                put("provider", value.provider.name)
            }
        }
    })
}.toString()

internal fun recoveryPayloadToJob(payload: String): AudioImportJob? = runCatching {
    val value = Json.parseToJsonElement(payload).jsonObject["androidAudioImportV1"]?.jsonObject
        ?: return@runCatching null
    val id = value.getValue("id").jsonPrimitive.content
    val target = when (value.getValue("target").jsonPrimitive.content) {
        "new" -> AudioImportTarget.NewImport(AudioImportSource(value.getValue("source").jsonPrimitive.content))
        "existing" -> AudioImportTarget.ExistingRecording(
            historyId = value.getValue("historyId").jsonPrimitive.content,
            audioPath = value.getValue("audioPath").jsonPrimitive.content,
            displayName = value["displayName"]?.jsonPrimitive?.contentOrNull.orEmpty(),
            durationMillis = value["durationMillis"]?.jsonPrimitive?.content?.toLongOrNull() ?: 0L,
        )
        else -> return@runCatching null
    }
    val engine = when (value.getValue("engine").jsonPrimitive.content) {
        "local" -> AudioImportEngine.Local(value.getValue("modelId").jsonPrimitive.content)
        "online" -> AudioImportEngine.Online(
            OnlineAsrProvider.valueOf(value.getValue("provider").jsonPrimitive.content),
        )
        else -> return@runCatching null
    }
    AudioImportJob(id, target, engine)
}.getOrNull()

private fun AudioImportStage.toRecoveryStage() = when (this) {
    AudioImportStage.QUEUED, AudioImportStage.STAGING -> RecoveryStage.QUEUED
    AudioImportStage.TRANSCODING -> RecoveryStage.TRANSCODING
    AudioImportStage.TRANSCRIBING -> RecoveryStage.TRANSCRIBING
    AudioImportStage.SAVING -> RecoveryStage.SAVING
}

private fun deleteRecoverySource(context: Context, path: String) {
    val file = File(path).canonicalFile
    val root = File(context.filesDir, "recovery/import-sources").canonicalFile
    if (isFileWithinRoot(file, root)) file.parentFile?.deleteRecursively()
}

internal fun AudioImportJob.toWorkData(displayName: String?): Data {
    val builder = Data.Builder()
        .putString(KEY_JOB_ID, id)
        .putString(KEY_DISPLAY_NAME, displayName)
    when (val jobTarget = target) {
        is AudioImportTarget.NewImport -> builder
            .putString(KEY_TARGET, TARGET_NEW)
            .putString(KEY_SOURCE_LOCATOR, jobTarget.source.locator)
        is AudioImportTarget.ExistingRecording -> builder
            .putString(KEY_TARGET, TARGET_EXISTING)
            .putString(KEY_HISTORY_ID, jobTarget.historyId)
            .putString(KEY_AUDIO_PATH, jobTarget.audioPath)
            .putLong(KEY_DURATION_MILLIS, jobTarget.durationMillis)
    }
    when (val snapshot = engine) {
        is AudioImportEngine.Local -> builder
            .putString(KEY_ENGINE, ENGINE_LOCAL)
            .putString(KEY_MODEL_ID, snapshot.modelId)
        is AudioImportEngine.Online -> builder
            .putString(KEY_ENGINE, ENGINE_ONLINE)
            .putString(KEY_PROVIDER, snapshot.provider.name)
    }
    return builder.build()
}

internal fun Data.toAudioImportJob(): AudioImportJob? {
    val jobId = getString(KEY_JOB_ID)?.takeIf { it.isNotBlank() } ?: return null
    val target = when (getString(KEY_TARGET)) {
        TARGET_NEW -> AudioImportTarget.NewImport(
            AudioImportSource(
                getString(KEY_SOURCE_LOCATOR)?.takeIf { it.isNotBlank() } ?: return null,
            ),
        )
        TARGET_EXISTING -> AudioImportTarget.ExistingRecording(
            historyId = getString(KEY_HISTORY_ID)?.takeIf { it.isNotBlank() } ?: return null,
            audioPath = getString(KEY_AUDIO_PATH)?.takeIf { it.isNotBlank() } ?: return null,
            displayName = getString(KEY_DISPLAY_NAME).orEmpty(),
            durationMillis = getLong(KEY_DURATION_MILLIS, 0L),
        )
        else -> return null
    }
    val engine = when (getString(KEY_ENGINE)) {
        ENGINE_LOCAL -> AudioImportEngine.Local(
            getString(KEY_MODEL_ID)?.takeIf { it.isNotBlank() } ?: return null,
        )
        ENGINE_ONLINE -> AudioImportEngine.Online(
            runCatching { OnlineAsrProvider.valueOf(getString(KEY_PROVIDER).orEmpty()) }
                .getOrNull() ?: return null,
        )
        else -> return null
    }
    return AudioImportJob(jobId, target, engine)
}

private fun progressData(
    job: AudioImportJob,
    displayName: String?,
    stage: AudioImportStage,
    percent: Int?,
): Data = workDataOf(
    KEY_JOB_ID to job.id,
    KEY_DISPLAY_NAME to displayName,
    KEY_STAGE to stage.name,
    KEY_PROGRESS_PERCENT to (percent ?: NO_PROGRESS),
)

private fun failureData(reason: AudioImportFailure, jobId: String?): Data = workDataOf(
    KEY_JOB_ID to jobId,
    KEY_FAILURE to reason.name,
)

private fun WorkInfo.toApplicationState(): AudioImportJobState {
    val jobId = progress.getString(KEY_JOB_ID)
        ?: outputData.getString(KEY_JOB_ID)
        ?: tags.firstOrNull { it.matches(Regex("[A-Za-z0-9-]{1,64}")) }
    return when (state) {
        WorkInfo.State.ENQUEUED, WorkInfo.State.BLOCKED -> AudioImportJobState.Running(
            jobId = jobId.orEmpty(),
            displayName = progress.getString(KEY_DISPLAY_NAME),
            stage = AudioImportStage.QUEUED,
            progressPercent = null,
        )
        WorkInfo.State.RUNNING -> AudioImportJobState.Running(
            jobId = jobId.orEmpty(),
            displayName = progress.getString(KEY_DISPLAY_NAME),
            stage = runCatching {
                AudioImportStage.valueOf(progress.getString(KEY_STAGE).orEmpty())
            }.getOrDefault(AudioImportStage.STAGING),
            progressPercent = progress.getInt(KEY_PROGRESS_PERCENT, NO_PROGRESS)
                .takeUnless { it == NO_PROGRESS },
        )
        WorkInfo.State.SUCCEEDED -> AudioImportJobState.Completed(
            jobId = jobId.orEmpty(),
            historyId = outputData.getString(KEY_HISTORY_ID).orEmpty(),
            transcriptionWarning = outputData.getBoolean(KEY_TRANSCRIPTION_WARNING, false),
        )
        WorkInfo.State.FAILED -> AudioImportJobState.Failed(
            jobId = jobId,
            reason = runCatching {
                AudioImportFailure.valueOf(outputData.getString(KEY_FAILURE).orEmpty())
            }.getOrDefault(AudioImportFailure.PERSISTENCE),
        )
        WorkInfo.State.CANCELLED -> AudioImportJobState.Idle
    }
}

private fun List<WorkInfo>.relevantAudioImport(): WorkInfo? =
    firstOrNull { !it.state.isFinished } ?: maxByOrNull { workInfo ->
        workInfo.tags.firstNotNullOfOrNull { tag ->
            if (tag.startsWith(ENQUEUED_AT_TAG_PREFIX)) {
                tag.removePrefix(ENQUEUED_AT_TAG_PREFIX).toLongOrNull()
            } else {
                null
            }
        } ?: Long.MIN_VALUE
    }

private fun AudioImportStage.notificationText(): String = when (this) {
    AudioImportStage.QUEUED -> "Audio import queued"
    AudioImportStage.STAGING -> "Preparing audio"
    AudioImportStage.TRANSCODING -> "Converting audio"
    AudioImportStage.TRANSCRIBING -> "Transcribing audio"
    AudioImportStage.SAVING -> "Saving audio"
}

private const val UNIQUE_WORK_NAME = "sona-audio-import"
private const val KEY_JOB_ID = "job_id"
private const val KEY_SOURCE_LOCATOR = "source_locator"
private const val KEY_TARGET = "target"
private const val KEY_DISPLAY_NAME = "display_name"
private const val KEY_ENGINE = "engine"
private const val KEY_MODEL_ID = "model_id"
private const val KEY_PROVIDER = "provider"
private const val KEY_AUDIO_PATH = "audio_path"
private const val KEY_DURATION_MILLIS = "duration_millis"
private const val KEY_STAGE = "stage"
private const val KEY_PROGRESS_PERCENT = "progress_percent"
private const val KEY_HISTORY_ID = "history_id"
private const val KEY_TRANSCRIPTION_WARNING = "transcription_warning"
private const val KEY_FAILURE = "failure"
private const val ENGINE_LOCAL = "local"
private const val ENGINE_ONLINE = "online"
private const val TARGET_NEW = "new"
private const val TARGET_EXISTING = "existing"
private const val MAX_RETRY_ATTEMPTS = 3
private const val NO_PROGRESS = -1
private const val NOTIFICATION_CHANNEL_ID = "audio_imports"
private const val NOTIFICATION_ID = 4102
private const val ENQUEUED_AT_TAG_PREFIX = "sona-audio-import-enqueued-at:"
