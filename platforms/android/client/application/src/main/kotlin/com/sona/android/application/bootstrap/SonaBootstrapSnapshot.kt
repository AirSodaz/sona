package com.sona.android.application.bootstrap

data class SonaBootstrapSnapshot(
    val defaultConfigJson: String,
    val onlineStreamingAvailable: Boolean,
    val localRuntimePackaged: Boolean,
    val localStreamingSessionAvailable: Boolean,
)
