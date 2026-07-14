package com.sona.android.adapters.uniffi.bootstrap

import com.sona.android.application.bootstrap.SonaBootstrapPort
import com.sona.android.application.bootstrap.SonaBootstrapSnapshot
import uniffi.sona_uniffi_bind.defaultConfigJson

class UniffiSonaBootstrapAdapter : SonaBootstrapPort {
    override fun load(): SonaBootstrapSnapshot = SonaBootstrapSnapshot(
        defaultConfigJson = defaultConfigJson(),
        onlineStreamingAvailable = true,
        localRuntimePackaged = true,
        localStreamingSessionAvailable = false,
    )
}
