package com.sona.android.app

import android.app.Application
import androidx.work.Configuration
import com.sona.android.app.composition.SonaAppContainer

class SonaApplication : Application(), Configuration.Provider {
    val container: SonaAppContainer by lazy { SonaAppContainer(this) }

    override fun onCreate() {
        super.onCreate()
        container.syncWork.schedulePeriodic()
    }

    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder()
            .setWorkerFactory(container.workerFactory)
            .build()
}
