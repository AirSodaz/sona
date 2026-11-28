package com.sona.android.adapters.android.settings

import com.sona.android.application.settings.AppApkAsset
import com.sona.android.application.settings.AppUpdateChannel
import com.sona.android.application.settings.AppUpdateDownloadProgress
import com.sona.android.application.settings.AppUpdateDownloadStage
import com.sona.android.application.settings.AppUpdateErrorCategory
import com.sona.android.application.settings.AppUpdateException
import com.sona.android.application.settings.DownloadMirror
import java.io.ByteArrayInputStream
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AndroidAppUpdateAdapterTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `stable and nightly use their fixed manifests and release pages`() = runTest {
        val requestedUrls = mutableListOf<String>()
        val connections = mutableListOf<FakeConnection>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            updatesDir = temporaryFolder.newFolder("updates"),
            connectionFactory = { url ->
                requestedUrls += url.toString()
                FakeConnection(
                    url = url,
                    response = manifest(
                        channel = if (requestedUrls.last().contains("nightly")) "nightly" else "stable",
                    ).toByteArray(),
                ).also(connections::add)
            },
            ioDispatcher = dispatcher,
        )

        val stable = adapter.latestRelease(AppUpdateChannel.STABLE, DownloadMirror.DIRECT)
        val nightly = adapter.latestRelease(AppUpdateChannel.NIGHTLY, DownloadMirror.DIRECT)

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
    fun `manifest parser accepts additive assets, notes, signing, and pubDate`() {
        val sha = "a".repeat(64)
        val release = parseUpdateManifest(
            payload = manifest(
                pubDate = "2026-11-28T12:00:00Z",
                notes = "Fix audio sync",
                signing = "debug",
                armUrl = "https://github.com/AirSodaz/sona/releases/download/v1.2.4/app-arm64-v8a-debug.apk",
                armSha = sha,
                armSize = 4096L,
            ),
            expectedChannel = AppUpdateChannel.STABLE,
            releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
        )

        assertEquals("1.2.4", release.versionName)
        assertEquals(42, release.versionCode)
        assertEquals("2026-11-28T12:00:00Z", release.pubDate)
        assertEquals("Fix audio sync", release.notes)
        assertEquals("debug", release.signing)
        val arm = release.assets["arm64-v8a"]
        assertNotNull(arm)
        assertEquals(sha, arm?.sha256)
        assertEquals(4096L, arm?.sizeBytes)
    }

    @Test
    fun `manifest check falls back to mirror when direct connection fails`() = runTest {
        val requestedUrls = mutableListOf<String>()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            updatesDir = temporaryFolder.newFolder("updates"),
            connectionFactory = { url ->
                requestedUrls += url.toString()
                if (!url.toString().startsWith("https://ghproxy.net/")) {
                    FakeConnection(url, ByteArray(0), responseCodeValue = 503)
                } else {
                    FakeConnection(url, manifest().toByteArray())
                }
            },
            ioDispatcher = dispatcher,
        )

        val release = adapter.latestRelease(AppUpdateChannel.STABLE, DownloadMirror.AUTO)
        assertEquals("1.2.4", release.versionName)
        assertEquals(2, requestedUrls.size)
        assertTrue(requestedUrls[1].startsWith("https://ghproxy.net/"))
    }

    @Test
    fun `apk download resumes partial file, verifies sha256, and caches final apk`() = runTest {
        val updatesDir = temporaryFolder.newFolder("updates")
        val payload = "sona-android-apk-bytes-0123456789".toByteArray()
        val sha = sha256Hex(payload)
        val partFile = File(updatesDir, "sona-update-42-arm64-v8a.apk.part")
        partFile.writeBytes(payload.copyOfRange(0, 10))
        val requestedRanges = mutableListOf<String?>()
        val progressEvents = mutableListOf<AppUpdateDownloadProgress>()
        val dispatcher = StandardTestDispatcher(testScheduler)

        val adapter = AndroidAppUpdateAdapter(
            updatesDir = updatesDir,
            connectionFactory = { url ->
                FakeConnection(
                    url = url,
                    response = payload.copyOfRange(10, payload.size),
                    responseCodeValue = HttpURLConnection.HTTP_PARTIAL,
                    contentLengthHeader = (payload.size - 10).toString(),
                    onConnect = { headers -> requestedRanges += headers["Range"] },
                )
            },
            ioDispatcher = dispatcher,
        )

        val asset = AppApkAsset(
            abi = "arm64-v8a",
            url = "https://github.com/AirSodaz/sona/releases/download/v1.2.4/app-arm64-v8a-debug.apk",
            sha256 = sha,
            sizeBytes = payload.size.toLong(),
        )
        val path = adapter.downloadApk(asset, 42, DownloadMirror.DIRECT) { progressEvents += it }

        assertEquals("bytes=10-", requestedRanges.single())
        assertTrue(File(path).isFile)
        assertEquals(payload.toList(), File(path).readBytes().toList())
        assertFalse(partFile.exists())
        assertEquals(AppUpdateDownloadStage.READY, progressEvents.last().stage)

        val release = parseUpdateManifest(
            payload = manifest(
                armUrl = asset.url,
                armSha = sha,
                armSize = payload.size.toLong(),
            ),
            expectedChannel = AppUpdateChannel.STABLE,
            releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
        )
        val cached = adapter.findCachedApk(release, listOf("arm64-v8a"))
        assertNotNull(cached)
        assertEquals(path, cached?.second)

        adapter.cleanupStaleUpdates(currentVersionCode = 42)
        assertNull(adapter.findCachedApk(release, listOf("arm64-v8a")))
    }

    @Test
    fun `apk download deletes corrupted payload on sha256 mismatch`() = runTest {
        val updatesDir = temporaryFolder.newFolder("updates")
        val payload = "corrupted-apk-bytes".toByteArray()
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            updatesDir = updatesDir,
            connectionFactory = { url ->
                FakeConnection(
                    url = url,
                    response = payload,
                    contentLengthHeader = payload.size.toString(),
                )
            },
            ioDispatcher = dispatcher,
        )
        val asset = AppApkAsset(
            abi = "arm64-v8a",
            url = "https://github.com/AirSodaz/sona/releases/download/v1.2.4/app-arm64-v8a-debug.apk",
            sha256 = "f".repeat(64),
            sizeBytes = payload.size.toLong(),
        )

        val error = runCatching {
            adapter.downloadApk(asset, 42, DownloadMirror.DIRECT) {}
        }.exceptionOrNull()

        assertTrue(error is AppUpdateException)
        assertEquals(
            AppUpdateErrorCategory.VERIFICATION_FAILED,
            (error as AppUpdateException).category,
        )
        assertTrue(updatesDir.listFiles().orEmpty().isEmpty())
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
            assertTrue(
                runCatching {
                    parseUpdateManifest(
                        payload = payload,
                        expectedChannel = AppUpdateChannel.STABLE,
                        releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
                    )
                }.isFailure,
            )
        }
    }

    @Test(expected = IOException::class)
    fun `http failures are rejected`() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            updatesDir = temporaryFolder.newFolder("updates"),
            connectionFactory = { url -> FakeConnection(url, ByteArray(0), responseCodeValue = 503) },
            ioDispatcher = dispatcher,
        )

        adapter.latestRelease(AppUpdateChannel.STABLE, DownloadMirror.DIRECT)
    }

    @Test(expected = IOException::class)
    fun `oversized responses are rejected`() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val adapter = AndroidAppUpdateAdapter(
            updatesDir = temporaryFolder.newFolder("updates"),
            connectionFactory = { url ->
                FakeConnection(
                    url,
                    "x".repeat(AndroidAppUpdateAdapter.MAX_RESPONSE_BYTES + 1).toByteArray(),
                )
            },
            ioDispatcher = dispatcher,
        )

        adapter.latestRelease(AppUpdateChannel.STABLE, DownloadMirror.DIRECT)
    }

    private class FakeConnection(
        url: URL,
        private val response: ByteArray,
        private val responseCodeValue: Int = HTTP_OK,
        private val contentLengthHeader: String? = null,
        private val onConnect: ((Map<String, String>) -> Unit)? = null,
    ) : HttpURLConnection(url) {
        private val headers = mutableMapOf<String, String>()

        override fun setRequestProperty(key: String?, value: String?) {
            if (key != null && value != null) {
                headers[key] = value
            }
            super.setRequestProperty(key, value)
        }

        override fun connect() = Unit
        override fun disconnect() = Unit
        override fun usingProxy(): Boolean = false
        override fun getResponseCode(): Int {
            onConnect?.invoke(headers)
            return responseCodeValue
        }

        override fun getHeaderField(name: String?): String? =
            if (name.equals("Content-Length", ignoreCase = true)) contentLengthHeader else null

        override fun getInputStream() = ByteArrayInputStream(response)
    }

    companion object {
        private fun sha256Hex(bytes: ByteArray): String =
            MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

        private fun manifest(
            schemaVersion: Int = 1,
            channel: String = "stable",
            versionName: String = "1.2.4",
            versionCode: Int = 42,
            pubDate: String? = null,
            notes: String? = null,
            signing: String? = null,
            armUrl: String? = null,
            armSha: String? = null,
            armSize: Long? = null,
        ): String {
            val fields = mutableListOf(
                """"schemaVersion": $schemaVersion""",
                """"channel": "$channel"""",
                """"versionName": "$versionName"""",
                """"versionCode": $versionCode""",
            )
            pubDate?.let { fields += """"pubDate": "$it"""" }
            notes?.let { fields += """"notes": "$it"""" }
            signing?.let { fields += """"signing": "$it"""" }
            if (armUrl != null && armSha != null && armSize != null) {
                fields += """
                    "assets": {
                      "arm64-v8a": {
                        "url": "$armUrl",
                        "sha256": "$armSha",
                        "sizeBytes": $armSize
                      }
                    }
                """.trimIndent()
            }
            return "{\n${fields.joinToString(",\n")}\n}"
        }
    }
}
