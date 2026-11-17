package com.sona.android.adapters.uniffi.logging

import android.util.Log
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

object UniffiMobileLoggerAdapter {
    fun initialize(isDebug: Boolean) {
        val level = if (isDebug) "debug" else "info"
        try {
            initMobileLogger(AndroidFfiLogCallback(), level)
            Log.i("UniffiMobileLogger", "Rust native logger bridge initialized with level $level")
        } catch (e: Throwable) {
            Log.w("UniffiMobileLogger", "Failed to initialize Rust native logger bridge: ${e.message}")
        }
    }
}
