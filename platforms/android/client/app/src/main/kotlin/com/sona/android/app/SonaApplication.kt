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
import uniffi.sona_uniffi_bind.FfiLogCallback
import uniffi.sona_uniffi_bind.initMobileLogger

private class AndroidFfiLogCallback : FfiLogCallback {
    override fun log(level: String, target: String, message: String) {
        val tag = "SonaNative:${target.substringAfterLast("::")}".take(23)
        when (level.uppercase()) {
            "ERROR" -> Log.e(tag, message)
            "WARN" -> Log.w(tag, message)
            "INFO" -> Log.i(tag, message)
            "DEBUG" -> Log.d(tag, message)
            "TRACE" -> Log.v(tag, message)
            else -> Log.i(tag, message)
        }
    }
}

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
            val level = if (BuildConfig.DEBUG) "debug" else "info"
            initMobileLogger(AndroidFfiLogCallback(), level)
            Log.i(TAG, "Rust native logger bridge initialized with level $level")
        } catch (e: Throwable) {
            Log.w(TAG, "Failed to initialize Rust native logger bridge: ${e.message}")
        }
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
