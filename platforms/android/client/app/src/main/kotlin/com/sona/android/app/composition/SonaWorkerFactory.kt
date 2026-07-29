package com.sona.android.app.composition

import android.content.Context
import androidx.work.ListenableWorker
import androidx.work.WorkerFactory
import androidx.work.WorkerParameters

internal class SonaWorkerFactory(
    private vararg val delegates: WorkerFactory,
) : WorkerFactory() {
    override fun createWorker(
        appContext: Context,
        workerClassName: String,
        workerParameters: WorkerParameters,
    ): ListenableWorker? = delegates.firstNotNullOfOrNull {
        it.createWorker(appContext, workerClassName, workerParameters)
    }
}
