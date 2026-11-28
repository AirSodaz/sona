package com.sona.android.adapters.uniffi.logging

import android.util.Log
import com.sona.android.application.settings.AppLogLevel
import uniffi.sona_uniffi_bind.FfiLogCallback
import uniffi.sona_uniffi_bind.initMobileLogger

fun interface MobileLogSink {
    fun append(level: String, target: String, message: String)
}

private class AndroidFfiLogCallback(
    private val sinkProvider: () -> MobileLogSink?,
) : FfiLogCallback {
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
        sinkProvider()?.append(level, target, message)
    }
}

object UniffiMobileLoggerAdapter {
    @Volatile
    private var logSink: MobileLogSink? = null
    private val callback = AndroidFfiLogCallback { logSink }

    fun initialize(isDebug: Boolean, sink: MobileLogSink? = null) {
        if (sink != null) {
            logSink = sink
        }
        val level = if (isDebug) AppLogLevel.DEBUG else AppLogLevel.INFO
        applyLevel(level)
    }

    fun setSink(sink: MobileLogSink?) {
        logSink = sink
    }

    fun applyLevel(level: AppLogLevel) {
        try {
            initMobileLogger(callback, level.wireValue)
            Log.i("UniffiMobileLogger", "Rust native logger bridge configured at level ${level.wireValue}")
        } catch (e: Throwable) {
            Log.w("UniffiMobileLogger", "Failed to configure Rust native logger bridge: ${e.message}")
        }
    }
}
