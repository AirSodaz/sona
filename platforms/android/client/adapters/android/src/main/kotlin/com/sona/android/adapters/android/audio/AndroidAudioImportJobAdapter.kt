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
import com.sona.android.application.recording.OnlineBatchProvider
import com.sona.android.application.recording.RunAudioImport
import com.sona.android.application.recording.RunAudioImportOutcome
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map

class AndroidAudioImportJobAdapter private constructor(
    private val context: Context,
    private val workManager: Lazy<WorkManager>,
) : AudioImportJobPort {
    companion object {
        fun create(context: Context): AndroidAudioImportJobAdapter = AndroidAudioImportJobAdapter(
            context = context.applicationContext,
            workManager = lazy { WorkManager.getInstance(context.applicationContext) },
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
        val displayName = when (val target = job.target) {
            is AudioImportTarget.NewImport -> {
                val uri = Uri.parse(target.source.locator)
                require(uri.scheme == "content") { "Audio import source must be a content URI." }
                context.contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION,
                )
                displayName(uri)
            }
            is AudioImportTarget.ExistingRecording -> target.displayName
        }
        val constraints = Constraints.Builder().apply {
            if (job.engine is AudioImportEngine.Online) {
                setRequiredNetworkType(NetworkType.CONNECTED)
            }
        }.build()
        val request = OneTimeWorkRequestBuilder<AudioImportWorker>()
            .setInputData(job.toWorkData(displayName))
            .setConstraints(constraints)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .addTag(job.id)
            .addTag("$ENQUEUED_AT_TAG_PREFIX${System.currentTimeMillis()}")
            .build()
        workManager.value.enqueueUniqueWork(
            UNIQUE_WORK_NAME,
            ExistingWorkPolicy.KEEP,
            request,
        )
    }

    override suspend fun cancel(jobId: String) {
        workManager.value.cancelAllWorkByTag(jobId)
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
}

class AudioImportWorkerFactory(
    private val runAudioImport: RunAudioImport,
) : WorkerFactory() {
    override fun createWorker(
        appContext: Context,
        workerClassName: String,
        workerParameters: WorkerParameters,
    ): ListenableWorker? = if (workerClassName == AudioImportWorker::class.java.name) {
        AudioImportWorker(appContext, workerParameters, runAudioImport)
    } else {
        null
    }
}

class AudioImportWorker internal constructor(
    appContext: Context,
    params: WorkerParameters,
    private val runAudioImport: RunAudioImport,
) : CoroutineWorker(appContext, params) {
    override suspend fun doWork(): Result {
        val job = inputData.toAudioImportJob()
            ?: return Result.failure(failureData(AudioImportFailure.INVALID_SOURCE, null))
        setForeground(foregroundInfo(AudioImportStage.STAGING, null))
        val displayName = inputData.getString(KEY_DISPLAY_NAME)
        val progress = AudioImportProgressListener { stage, percent ->
            setProgress(progressData(job, displayName, stage, percent))
            setForeground(foregroundInfo(stage, percent))
        }
        return when (
            val outcome = runAudioImport(
                job = job,
                progress = progress,
                allowTranscriptionWarning = runAttemptCount >= MAX_RETRY_ATTEMPTS - 1,
            )
        ) {
            is RunAudioImportOutcome.Completed -> Result.success(
                workDataOf(
                    KEY_JOB_ID to job.id,
                    KEY_HISTORY_ID to outcome.historyId,
                    KEY_TRANSCRIPTION_WARNING to outcome.transcriptionWarning,
                ),
            )
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
            runCatching { OnlineBatchProvider.valueOf(getString(KEY_PROVIDER).orEmpty()) }
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
