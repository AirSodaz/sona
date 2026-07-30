package com.sona.android.adapters.android.settings

import com.sona.android.application.settings.AppReleaseInfo
import com.sona.android.application.settings.AppUpdateChannel
import com.sona.android.application.settings.AppUpdatePort
import com.sona.android.application.settings.MAXIMUM_ANDROID_VERSION_CODE
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

class AndroidAppUpdateAdapter internal constructor(
    private val connectionFactory: (URL) -> HttpURLConnection,
    private val ioDispatcher: CoroutineDispatcher,
) : AppUpdatePort {
    constructor() : this(
        connectionFactory = { url -> url.openConnection() as HttpURLConnection },
        ioDispatcher = Dispatchers.IO,
    )

    override suspend fun latestRelease(channel: AppUpdateChannel): AppReleaseInfo =
        withContext(ioDispatcher) {
            val endpoint = endpointFor(channel)
            val connection = connectionFactory(URL(endpoint.manifestUrl))
            try {
                connection.requestMethod = "GET"
                connection.instanceFollowRedirects = true
                connection.connectTimeout = CONNECT_TIMEOUT_MILLIS
                connection.readTimeout = READ_TIMEOUT_MILLIS
                connection.useCaches = false
                connection.setRequestProperty("Accept", "application/json")
                connection.setRequestProperty("User-Agent", "Sona-Android-Update")

                val responseCode = connection.responseCode
                if (responseCode !in 200..299) {
                    throw IOException("Unexpected update response")
                }
                val payload = connection.inputStream.use(::readLimitedUtf8)
                parseUpdateManifest(
                    payload = payload,
                    expectedChannel = channel,
                    releasePageUrl = endpoint.releasePageUrl,
                )
            } finally {
                connection.disconnect()
            }
        }

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

    companion object {
        private const val CONNECT_TIMEOUT_MILLIS = 5_000
        private const val READ_TIMEOUT_MILLIS = 10_000
        internal const val MAX_RESPONSE_BYTES = 64 * 1024
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
    return AppReleaseInfo(
        versionName = versionName,
        versionCode = versionCode,
        channel = channel,
        releasePageUrl = releasePageUrl,
    )
}
