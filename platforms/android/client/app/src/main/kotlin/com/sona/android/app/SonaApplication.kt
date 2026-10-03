package com.sona.android.app

import android.app.Application
import android.content.Context
import android.util.Log
import androidx.work.Configuration
import com.sona.android.app.composition.SonaAppContainer
import com.sona.android.app.notification.SonaNotificationChannels
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class SonaApplication : Application(), Configuration.Provider {
    companion object {
        private const val TAG = "SonaApplication"

        @Volatile
        var isNativeTlsInitialized: Boolean = false
            private set

        init {
            System.loadLibrary("sona_uniffi_bind")
        }

        @JvmStatic
        private external fun initRustlsPlatformVerifier(context: Context): Boolean
    }

    val container: SonaAppContainer by lazy { SonaAppContainer(this) }

    override fun onCreate() {
        super.onCreate()
        try {
            isNativeTlsInitialized = initRustlsPlatformVerifier(this)
            check(isNativeTlsInitialized) {
                "Native TLS certificate verifier (rustls-platform-verifier) failed to initialize"
            }
            Log.i(TAG, "rustls-platform-verifier successfully initialized")
        } catch (e: Throwable) {
            isNativeTlsInitialized = false
            throw IllegalStateException("Failed to initialize native TLS verifier: ${e.message}", e)
        }
        SonaNotificationChannels.createChannels(this)
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
