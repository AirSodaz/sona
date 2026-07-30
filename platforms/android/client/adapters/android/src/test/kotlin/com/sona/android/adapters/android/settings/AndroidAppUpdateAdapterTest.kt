package com.sona.android.adapters.android.settings

import com.sona.android.application.settings.AppUpdateChannel
import java.io.ByteArrayInputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidAppUpdateAdapterTest {
    @Test
    fun `stable and nightly use their fixed manifests and release pages`() = runTest {
        val requestedUrls = mutableListOf<String>()
        val connections = mutableListOf<FakeConnection>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            connectionFactory = { url ->
                requestedUrls += url.toString()
                FakeConnection(url, manifest(channel = requestedUrls.last().contains("nightly").let {
                    if (it) "nightly" else "stable"
                })).also(connections::add)
            },
            ioDispatcher = dispatcher,
        )

        val stable = adapter.latestRelease(AppUpdateChannel.STABLE)
        val nightly = adapter.latestRelease(AppUpdateChannel.NIGHTLY)

        assertEquals(
            "https://github.com/AirSodaz/sona/releases/latest/download/android-update.json",
            requestedUrls[0],
        )
        assertEquals(
            "https://github.com/AirSodaz/sona/releases/download/nightly/android-update.json",
            requestedUrls[1],
        )
        assertEquals("https://github.com/AirSodaz/sona/releases/latest", stable.releasePageUrl)
        assertEquals("https://github.com/AirSodaz/sona/releases/tag/nightly", nightly.releasePageUrl)
        connections.forEach { connection ->
            assertEquals(5_000, connection.connectTimeout)
            assertEquals(10_000, connection.readTimeout)
        }
    }

    @Test
    fun `manifest parser accepts schema one`() {
        val release = parseUpdateManifest(
            payload = manifest(),
            expectedChannel = AppUpdateChannel.STABLE,
            releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
        )

        assertEquals("0.8.1", release.versionName)
        assertEquals(42, release.versionCode)
    }

    @Test
    fun `manifest parser rejects malformed schema channel and version values`() {
        val invalidPayloads = listOf(
            "not json",
            manifest(schemaVersion = 2),
            manifest(channel = "nightly"),
            manifest(versionCode = 0),
            manifest(versionName = ""),
        )

        invalidPayloads.forEach { payload ->
            assertTrue(runCatching {
                parseUpdateManifest(
                    payload = payload,
                    expectedChannel = AppUpdateChannel.STABLE,
                    releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
                )
            }.isFailure)
        }
    }

    @Test(expected = IOException::class)
    fun `http failures are rejected`() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            connectionFactory = { url -> FakeConnection(url, "", responseCodeValue = 503) },
            ioDispatcher = dispatcher,
        )

        adapter.latestRelease(AppUpdateChannel.STABLE)
    }

    @Test(expected = IOException::class)
    fun `oversized responses are rejected`() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            connectionFactory = { url ->
                FakeConnection(url, "x".repeat(AndroidAppUpdateAdapter.MAX_RESPONSE_BYTES + 1))
            },
            ioDispatcher = dispatcher,
        )

        adapter.latestRelease(AppUpdateChannel.STABLE)
    }

    private class FakeConnection(
        url: URL,
        private val response: String,
        private val responseCodeValue: Int = HTTP_OK,
    ) : HttpURLConnection(url) {
        override fun connect() = Unit
        override fun disconnect() = Unit
        override fun usingProxy(): Boolean = false
        override fun getResponseCode(): Int = responseCodeValue
        override fun getInputStream() = ByteArrayInputStream(response.toByteArray())
    }

    companion object {
        private fun manifest(
            schemaVersion: Int = 1,
            channel: String = "stable",
            versionName: String = "0.8.1",
            versionCode: Int = 42,
        ): String = """
            {
              "schemaVersion": $schemaVersion,
              "channel": "$channel",
              "versionName": "$versionName",
              "versionCode": $versionCode
            }
        """.trimIndent()
    }
}
