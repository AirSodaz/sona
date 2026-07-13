package com.sona.android.app

import com.sona.android.adapters.uniffi.UniffiSonaBootstrapAdapter
import com.sona.android.application.bootstrap.LoadSonaBootstrap

class SonaAppContainer {
    private val bootstrapPort = UniffiSonaBootstrapAdapter()

    val loadSonaBootstrap = LoadSonaBootstrap(bootstrapPort)
}
