package com.sona.android.adapters.android.settings

import android.content.Context
import com.sona.android.application.settings.AppApkAsset
import com.sona.android.application.settings.AppReleaseInfo
import com.sona.android.application.settings.AppUpdateChannel
import com.sona.android.application.settings.AppUpdateDownloadPort
import com.sona.android.application.settings.AppUpdateDownloadProgress
import com.sona.android.application.settings.AppUpdateDownloadStage
import com.sona.android.application.settings.AppUpdateErrorCategory
import com.sona.android.application.settings.AppUpdateException
import com.sona.android.application.settings.AppUpdatePort
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.DownloadUrlResolver
import com.sona.android.application.settings.MAXIMUM_ANDROID_VERSION_CODE
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlin.coroutines.coroutineContext
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

class AndroidAppUpdateAdapter internal constructor(
    private val updatesDir: File,
    private val connectionFactory: (URL) -> HttpURLConnection,
    private val ioDispatcher: CoroutineDispatcher,
) : AppUpdatePort, AppUpdateDownloadPort {
    constructor(context: Context) : this(
        updatesDir = File(context.applicationContext.cacheDir, "updates"),
        connectionFactory = { url -> url.openConnection() as HttpURLConnection },
        ioDispatcher = Dispatchers.IO,
    )

    override suspend fun latestRelease(
        channel: AppUpdateChannel,
        mirror: DownloadMirror,
    ): AppReleaseInfo = withContext(ioDispatcher) {
        val endpoint = endpointFor(channel)
        val candidates = DownloadUrlResolver.candidatesForGitHubUrl(endpoint.manifestUrl, mirror)
        var lastError: Throwable? = null
        for (candidateUrl in candidates) {
            coroutineContext.ensureActive()
            try {
                val payload = fetchManifestUtf8(candidateUrl)
                return@withContext parseUpdateManifest(
                    payload = payload,
                    expectedChannel = channel,
                    releasePageUrl = endpoint.releasePageUrl,
                )
            } catch (error: Throwable) {
                lastError = error
            }
        }
        throw (lastError ?: IOException("Failed to fetch update manifest"))
    }

    private fun fetchManifestUtf8(manifestUrl: String): String {
        val connection = connectionFactory(URL(manifestUrl))
        return try {
            connection.requestMethod = "GET"
            connection.instanceFollowRedirects = true
            connection.connectTimeout = CONNECT_TIMEOUT_MILLIS
            connection.readTimeout = READ_TIMEOUT_MILLIS
            connection.useCaches = false
            connection.setRequestProperty("Accept", "application/json")
            connection.setRequestProperty("User-Agent", "Sona-Android-Update")

            val responseCode = connection.responseCode
            if (responseCode !in 200..299) {
                throw IOException("Unexpected update response ($responseCode)")
            }
            connection.inputStream.use(::readLimitedUtf8)
        } finally {
            connection.disconnect()
        }
    }

    override suspend fun downloadApk(
        asset: AppApkAsset,
        versionCode: Int,
        mirror: DownloadMirror,
        onProgress: (AppUpdateDownloadProgress) -> Unit,
    ): String = withContext(ioDispatcher) {
        updatesDir.mkdirs()
        val finalFile = apkFile(versionCode, asset.abi)
        if (finalFile.isFile && verifyFileSha256(finalFile, asset.sha256)) {
            onProgress(
                AppUpdateDownloadProgress(
                    downloadedBytes = finalFile.length(),
                    totalBytes = asset.sizeBytes,
                    stage = AppUpdateDownloadStage.READY,
                ),
            )
            return@withContext finalFile.absolutePath
        }
        if (finalFile.exists()) {
            finalFile.delete()
        }

        val partFile = File(updatesDir, "${finalFile.name}.part")
        val candidates = DownloadUrlResolver.candidatesForGitHubUrl(asset.url, mirror)
        var lastError: Throwable? = null
        for (candidateUrl in candidates) {
            coroutineContext.ensureActive()
            try {
                downloadSingleCandidate(
                    candidateUrl = candidateUrl,
                    asset = asset,
                    partFile = partFile,
                    finalFile = finalFile,
                    onProgress = onProgress,
                )
                return@withContext finalFile.absolutePath
            } catch (error: AppUpdateException) {
                if (error.category == AppUpdateErrorCategory.VERIFICATION_FAILED) {
                    partFile.delete()
                }
                lastError = error
            } catch (error: Throwable) {
                if (error is kotlinx.coroutines.CancellationException) throw error
                lastError = error
            }
        }

        val exception = lastError
        if (exception is AppUpdateException) throw exception
        throw AppUpdateException(
            AppUpdateErrorCategory.NETWORK,
            exception?.message ?: "Failed to download update package",
            exception,
        )
    }

    private suspend fun downloadSingleCandidate(
        candidateUrl: String,
        asset: AppApkAsset,
        partFile: File,
        finalFile: File,
        onProgress: (AppUpdateDownloadProgress) -> Unit,
    ) {
        val existingBytes = if (partFile.isFile) partFile.length() else 0L
        val shouldResume = existingBytes in 1L until asset.sizeBytes
        val connection = connectionFactory(URL(candidateUrl))
        try {
            connection.requestMethod = "GET"
            connection.instanceFollowRedirects = true
            connection.connectTimeout = CONNECT_TIMEOUT_MILLIS
            connection.readTimeout = DOWNLOAD_READ_TIMEOUT_MILLIS
            connection.useCaches = false
            connection.setRequestProperty("User-Agent", "Sona-Android-Update")
            if (shouldResume) {
                connection.setRequestProperty("Range", "bytes=$existingBytes-")
            }

            val responseCode = connection.responseCode
            if (responseCode == HTTP_RANGE_NOT_SATISFIABLE) {
                partFile.delete()
                throw IOException("Range not satisfiable; restarting download")
            }
            if (responseCode !in 200..299) {
                throw IOException("Unexpected APK download status ($responseCode)")
            }

            val append = shouldResume && responseCode == HttpURLConnection.HTTP_PARTIAL
            val startingBytes = if (append) existingBytes else 0L
            val contentLength = connection.contentLengthLongCompat()
            val expectedTotal = when {
                append && contentLength > 0L -> startingBytes + contentLength
                !append && contentLength > 0L -> contentLength
                else -> asset.sizeBytes
            }

            var downloaded = startingBytes
            var lastReportedBytes = -1L
            var lastReportMs = 0L

            onProgress(
                AppUpdateDownloadProgress(
                    downloadedBytes = downloaded,
                    totalBytes = expectedTotal,
                    stage = AppUpdateDownloadStage.DOWNLOADING,
                ),
            )

            connection.inputStream.use { input ->
                FileOutputStream(partFile, append).use { output ->
                    val buffer = ByteArray(DOWNLOAD_BUFFER_BYTES)
                    while (true) {
                        coroutineContext.ensureActive()
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        downloaded += read
                        val now = System.currentTimeMillis()
                        if (downloaded - lastReportedBytes >= PROGRESS_BYTE_STEP ||
                            now - lastReportMs >= PROGRESS_TIME_STEP_MS
                        ) {
                            lastReportedBytes = downloaded
                            lastReportMs = now
                            onProgress(
                                AppUpdateDownloadProgress(
                                    downloadedBytes = downloaded,
                                    totalBytes = expectedTotal,
                                    stage = AppUpdateDownloadStage.DOWNLOADING,
                                ),
                            )
                        }
                    }
                    output.flush()
                }
            }

            onProgress(
                AppUpdateDownloadProgress(
                    downloadedBytes = partFile.length(),
                    totalBytes = expectedTotal,
                    stage = AppUpdateDownloadStage.VERIFYING,
                ),
            )

            if (!verifyFileSha256(partFile, asset.sha256)) {
                partFile.delete()
                throw AppUpdateException(
                    AppUpdateErrorCategory.VERIFICATION_FAILED,
                    "Downloaded APK checksum verification failed",
                )
            }

            if (finalFile.exists() && !finalFile.delete()) {
                throw IOException("Unable to replace existing cached APK")
            }
            if (!partFile.renameTo(finalFile)) {
                partFile.copyTo(finalFile, overwrite = true)
                partFile.delete()
            }

            onProgress(
                AppUpdateDownloadProgress(
                    downloadedBytes = finalFile.length(),
                    totalBytes = asset.sizeBytes,
                    stage = AppUpdateDownloadStage.READY,
                ),
            )
        } finally {
            connection.disconnect()
        }
    }

    override fun findCachedApk(
        release: AppReleaseInfo,
        supportedAbis: List<String>,
    ): Pair<AppApkAsset, String>? {
        val asset = release.selectAssetForAbis(supportedAbis) ?: return null
        val file = apkFile(release.versionCode, asset.abi)
        if (!file.isFile) return null
        return if (verifyFileSha256(file, asset.sha256)) {
            asset to file.absolutePath
        } else {
            file.delete()
            null
        }
    }

    override suspend fun cleanupStaleUpdates(currentVersionCode: Int) {
        withContext(ioDispatcher) {
            if (!updatesDir.isDirectory) return@withContext
            updatesDir.listFiles()?.forEach { file ->
                val match = VERSION_FILE_REGEX.matchEntire(file.name)
                if (match == null) {
                    file.delete()
                } else {
                    val fileVersionCode = match.groupValues[1].toIntOrNull() ?: 0
                    if (fileVersionCode <= currentVersionCode) {
                        file.delete()
                    }
                }
            }
        }
    }

    override suspend fun clearCachedUpdates(): Long = withContext(ioDispatcher) {
        if (!updatesDir.isDirectory) return@withContext 0L
        var freed = 0L
        updatesDir.listFiles()?.forEach { file ->
            if (file.isFile) {
                val len = file.length()
                if (file.delete()) {
                    freed += len
                }
            }
        }
        freed
    }

    private fun apkFile(versionCode: Int, abi: String): File =
        File(updatesDir, "sona-update-$versionCode-$abi.apk")

    private fun readLimitedUtf8(input: java.io.InputStream): String {
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
        var total = 0
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            total += read
            if (total > MAX_RESPONSE_BYTES) {
                throw IOException("Update response is too large")
            }
            output.write(buffer, 0, read)
        }
        return output.toString(Charsets.UTF_8.name())
    }

    private fun verifyFileSha256(file: File, expectedHex: String): Boolean = runCatching {
        val digest = MessageDigest.getInstance("SHA-256")
        val buffer = ByteArray(DOWNLOAD_BUFFER_BYTES)
        file.inputStream().use { input ->
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        val actualHex = digest.digest().joinToString("") { "%02x".format(it) }
        actualHex.equals(expectedHex, ignoreCase = true)
    }.getOrDefault(false)

    private fun HttpURLConnection.contentLengthLongCompat(): Long =
        getHeaderField("Content-Length")?.trim()?.toLongOrNull() ?: -1L

    companion object {
        private const val CONNECT_TIMEOUT_MILLIS = 5_000
        private const val READ_TIMEOUT_MILLIS = 10_000
        private const val DOWNLOAD_READ_TIMEOUT_MILLIS = 20_000
        private const val DOWNLOAD_BUFFER_BYTES = 16 * 1024
        private const val PROGRESS_BYTE_STEP = 64 * 1024L
        private const val PROGRESS_TIME_STEP_MS = 150L
        private const val HTTP_RANGE_NOT_SATISFIABLE = 416
        internal const val MAX_RESPONSE_BYTES = 64 * 1024
        private val VERSION_FILE_REGEX = Regex("""sona-update-(\d+)-[^.]+\.apk(?:\.part)?""")
    }
}

private data class UpdateEndpoint(
    val manifestUrl: String,
    val releasePageUrl: String,
)

private fun endpointFor(channel: AppUpdateChannel): UpdateEndpoint = when (channel) {
    AppUpdateChannel.STABLE -> UpdateEndpoint(
        manifestUrl = "https://github.com/AirSodaz/sona/releases/latest/download/android-update.json",
        releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
    )
    AppUpdateChannel.NIGHTLY -> UpdateEndpoint(
        manifestUrl = "https://github.com/AirSodaz/sona/releases/download/nightly/android-update.json",
        releasePageUrl = "https://github.com/AirSodaz/sona/releases/tag/nightly",
    )
    AppUpdateChannel.CI,
    AppUpdateChannel.DEV -> throw IllegalArgumentException("Auto-update is not supported for ${channel.wireValue} channel")
}

internal fun parseUpdateManifest(
    payload: String,
    expectedChannel: AppUpdateChannel,
    releasePageUrl: String,
): AppReleaseInfo {
    val manifest = Json.parseToJsonElement(payload).jsonObject
    require(manifest["schemaVersion"]?.jsonPrimitive?.intOrNull == 1) {
        "Unsupported update manifest schema"
    }
    val channelValue = manifest["channel"]?.jsonPrimitive?.contentOrNull
        ?: throw IllegalArgumentException("Missing update channel")
    val channel = AppUpdateChannel.fromWireValue(channelValue)
    require(channel == expectedChannel) { "Unexpected update channel" }
    val versionName = manifest["versionName"]?.jsonPrimitive?.contentOrNull
        ?.trim()
        ?.takeIf(String::isNotEmpty)
        ?: throw IllegalArgumentException("Missing update version name")
    require(versionName.length <= 100) { "Update version name is too long" }
    val versionCode = manifest["versionCode"]?.jsonPrimitive?.intOrNull
        ?: throw IllegalArgumentException("Missing update version code")
    require(versionCode in 1..MAXIMUM_ANDROID_VERSION_CODE) {
        "Invalid update version code"
    }

    val pubDate = manifest["pubDate"]?.jsonPrimitive?.contentOrNull?.trim()?.takeIf(String::isNotEmpty)
    val notes = manifest["notes"]?.jsonPrimitive?.contentOrNull?.trim()?.takeIf(String::isNotEmpty)
    val signing = manifest["signing"]?.jsonPrimitive?.contentOrNull?.trim()?.takeIf(String::isNotEmpty)
    val assets = (manifest["assets"] as? JsonObject)?.let(::parseAssets).orEmpty()

    return AppReleaseInfo(
        versionName = versionName,
        versionCode = versionCode,
        channel = channel,
        releasePageUrl = releasePageUrl,
        pubDate = pubDate,
        notes = notes,
        signing = signing,
        assets = assets,
    )
}

private fun parseAssets(assetsObj: JsonObject): Map<String, AppApkAsset> {
    val result = linkedMapOf<String, AppApkAsset>()
    for ((abi, element) in assetsObj) {
        val obj = element as? JsonObject ?: continue
        val url = obj["url"]?.jsonPrimitive?.contentOrNull?.trim().orEmpty()
        val sha256 = obj["sha256"]?.jsonPrimitive?.contentOrNull?.trim()?.lowercase().orEmpty()
        val sizeBytes = obj["sizeBytes"]?.jsonPrimitive?.longOrNull ?: 0L
        if (url.startsWith("https://") &&
            sha256.matches(Regex("[0-9a-f]{64}")) &&
            sizeBytes > 0L
        ) {
            result[abi] = AppApkAsset(
                abi = abi,
                url = url,
                sha256 = sha256,
                sizeBytes = sizeBytes,
            )
        }
    }
    return result
}
