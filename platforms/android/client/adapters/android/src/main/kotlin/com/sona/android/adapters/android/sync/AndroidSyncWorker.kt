package com.sona.android.adapters.android.sync

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.ListenableWorker
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerFactory
import androidx.work.WorkerParameters
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncSchedulerPort
import java.util.concurrent.TimeUnit

class AndroidSyncScheduler private constructor(
    private val workManager: Lazy<WorkManager>,
) : SyncSchedulerPort {
    override fun schedulePeriodic() {
        workManager.value.enqueueUniquePeriodicWork(
            PERIODIC_WORK,
            ExistingPeriodicWorkPolicy.UPDATE,
            PeriodicWorkRequestBuilder<SyncWorker>(6, TimeUnit.HOURS)
                .setConstraints(syncNetworkConstraints())
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build(),
        )
    }

    override fun scheduleAfterLocalChange() {
        workManager.value.enqueueUniqueWork(
            LOCAL_CHANGE_WORK,
            ExistingWorkPolicy.REPLACE,
            OneTimeWorkRequestBuilder<SyncWorker>()
                .setInitialDelay(15, TimeUnit.SECONDS)
                .setConstraints(syncNetworkConstraints())
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build(),
        )
    }

    override fun scheduleImmediate() {
        workManager.value.enqueueUniqueWork(
            MANUAL_WORK,
            ExistingWorkPolicy.REPLACE,
            OneTimeWorkRequestBuilder<SyncWorker>()
                .setConstraints(syncNetworkConstraints())
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build(),
        )
    }

    override fun cancelAll() {
        workManager.value.cancelUniqueWork(PERIODIC_WORK)
        workManager.value.cancelUniqueWork(LOCAL_CHANGE_WORK)
        workManager.value.cancelUniqueWork(MANUAL_WORK)
    }

    companion object {
        fun create(context: Context): AndroidSyncScheduler =
            AndroidSyncScheduler(lazy { WorkManager.getInstance(context.applicationContext) })
    }
}

class SyncWorker(
    appContext: Context,
    params: WorkerParameters,
    private val sync: SyncPort,
) : CoroutineWorker(appContext, params) {
    override suspend fun doWork(): Result = when (runSyncWork(sync)) {
        SyncWorkOutcome.SUCCESS -> Result.success()
        SyncWorkOutcome.RETRY -> Result.retry()
        SyncWorkOutcome.FAILURE -> Result.failure()
    }
}

internal enum class SyncWorkOutcome { SUCCESS, RETRY, FAILURE }

internal suspend fun runSyncWork(sync: SyncPort): SyncWorkOutcome {
    val status = try {
        sync.status()
    } catch (_: Exception) {
        return SyncWorkOutcome.RETRY
    }
    if (status.state in setOf(
            SyncLifecycleState.DISABLED,
            SyncLifecycleState.LOCKED,
            SyncLifecycleState.PAUSED,
            SyncLifecycleState.SYNCING,
        )
    ) return SyncWorkOutcome.SUCCESS
    if (status.state == SyncLifecycleState.ERROR) {
        return if (status.lastError?.retryable == true) {
            SyncWorkOutcome.RETRY
        } else {
            SyncWorkOutcome.FAILURE
        }
    }

    return try {
        sync.runNow()
        SyncWorkOutcome.SUCCESS
    } catch (_: Exception) {
        val refreshed = runCatching { sync.status() }.getOrNull()
        if (refreshed?.lastError?.retryable == true) SyncWorkOutcome.RETRY else SyncWorkOutcome.FAILURE
    }
}

class SyncWorkerFactory(private val sync: SyncPort) : WorkerFactory() {
    override fun createWorker(
        appContext: Context,
        workerClassName: String,
        workerParameters: WorkerParameters,
    ): ListenableWorker? = if (workerClassName == SyncWorker::class.java.name) {
        SyncWorker(appContext, workerParameters, sync)
    } else {
        null
    }
}

private const val PERIODIC_WORK = "sona-sync-periodic"
private const val LOCAL_CHANGE_WORK = "sona-sync-local-change"
private const val MANUAL_WORK = "sona-sync-manual"

internal fun syncNetworkConstraints() = Constraints.Builder()
    .setRequiredNetworkType(NetworkType.CONNECTED)
    .build()
