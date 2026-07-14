package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.StreamingProviderCatalogPort
import com.sona.android.application.recording.StreamingProviderProfile

class UniffiStreamingProviderCatalogAdapter internal constructor(
    private val bindings: UniffiProviderBindings,
) : StreamingProviderCatalogPort {
    constructor() : this(GeneratedUniffiProviderBindings)

    override suspend fun loadVolcengineStreamingProfile(): StreamingProviderProfile {
        val provider = checkNotNull(bindings.findProvider(VOLCENGINE_PROVIDER_ID)) {
            "Volcengine streaming provider is unavailable."
        }
        check(provider.streamingSupported != false) {
            "Volcengine streaming provider is disabled."
        }
        val config = bindings.parseVolcengineConfig(provider.defaultsJson)
        return StreamingProviderProfile(
            providerId = provider.providerId,
            profileId = provider.profileId,
            streamingEndpoint = config.streamingEndpoint,
            streamingResourceId = config.streamingResourceId,
        )
    }

    private companion object {
        const val VOLCENGINE_PROVIDER_ID = "volcengine-doubao"
    }
}
