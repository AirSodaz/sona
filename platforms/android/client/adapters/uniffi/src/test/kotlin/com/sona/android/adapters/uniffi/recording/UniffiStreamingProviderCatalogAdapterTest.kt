package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.StreamingProviderProfile
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class UniffiStreamingProviderCatalogAdapterTest {
    @Test
    fun `loads the typed Volcengine profile from the UniFFI manifest defaults`() = runTest {
        val bindings = FakeProviderBindings()
        val adapter = UniffiStreamingProviderCatalogAdapter(bindings)

        val profile = adapter.loadVolcengineStreamingProfile()

        assertEquals(
            StreamingProviderProfile(
                providerId = "volcengine-doubao",
                profileId = "volcengine-doubao-default",
                streamingEndpoint = "wss://stream.example",
                streamingResourceId = "stream-resource",
            ),
            profile,
        )
        assertEquals(listOf("volcengine-doubao"), bindings.providerIds)
        assertEquals(listOf("{\"streamingEndpoint\":\"wss://stream\"}"), bindings.configJson)
    }

    @Test
    fun `missing or explicitly disabled providers are rejected`() {
        val missing = FakeProviderBindings().apply { manifest = null }
        assertThrows(IllegalStateException::class.java) {
            kotlinx.coroutines.test.runTest {
                UniffiStreamingProviderCatalogAdapter(missing)
                    .loadVolcengineStreamingProfile()
            }
        }

        val disabled = FakeProviderBindings().apply {
            manifest = manifest?.copy(streamingSupported = false)
        }
        assertThrows(IllegalStateException::class.java) {
            kotlinx.coroutines.test.runTest {
                UniffiStreamingProviderCatalogAdapter(disabled)
                    .loadVolcengineStreamingProfile()
            }
        }
    }

    private class FakeProviderBindings : UniffiProviderBindings {
        val providerIds = mutableListOf<String>()
        val configJson = mutableListOf<String>()
        var manifest: UniffiStreamingProviderManifest? = UniffiStreamingProviderManifest(
            providerId = "volcengine-doubao",
            profileId = "volcengine-doubao-default",
            defaultsJson = "{\"streamingEndpoint\":\"wss://stream\"}",
            streamingSupported = null,
        )

        override fun findProvider(providerId: String): UniffiStreamingProviderManifest? {
            providerIds += providerId
            return manifest
        }

        override fun parseVolcengineConfig(configJson: String): UniffiVolcengineStreamingConfig {
            this.configJson += configJson
            return UniffiVolcengineStreamingConfig(
                streamingEndpoint = "wss://stream.example",
                streamingResourceId = "stream-resource",
            )
        }
    }
}
