package com.sona.android.application.bootstrap

fun interface SonaBootstrapPort {
    fun load(): SonaBootstrapSnapshot
}
