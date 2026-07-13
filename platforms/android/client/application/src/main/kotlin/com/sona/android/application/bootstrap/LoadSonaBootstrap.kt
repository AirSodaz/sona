package com.sona.android.application.bootstrap

class LoadSonaBootstrap(
    private val port: SonaBootstrapPort,
) {
    operator fun invoke(): SonaBootstrapSnapshot {
        val snapshot = port.load()
        require(snapshot.defaultConfigJson.isNotBlank()) {
            "Default config JSON must not be blank"
        }
        return snapshot
    }
}
