package com.sona.android.app.composition

import com.sona.android.adapters.uniffi.bootstrap.UniffiSonaBootstrapAdapter
import com.sona.android.application.bootstrap.LoadSonaBootstrap

class SonaAppContainer {
    private val bootstrapPort = UniffiSonaBootstrapAdapter()

    val loadSonaBootstrap = LoadSonaBootstrap(bootstrapPort)
}
