package com.sona.android.app

import android.app.Application
import androidx.work.Configuration
import com.sona.android.app.composition.SonaAppContainer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class SonaApplication : Application(), Configuration.Provider {
    val container: SonaAppContainer by lazy { SonaAppContainer(this) }

    override fun onCreate() {
        super.onCreate()
        container.syncWork.schedulePeriodic()
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            container.audioImportJobsController.reconcileAndSchedule()
        }
    }

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder()
            .setWorkerFactory(container.workerFactory)
            .build()
}
