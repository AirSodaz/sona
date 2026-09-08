package com.sona.android.application.sync

enum class SyncPreset { CONTENT, STANDARD, FULL }
enum class SyncLifecycleState { DISABLED, LOCKED, IDLE, SYNCING, PAUSED, ERROR }
enum class SyncConflictResolution { KEEP_CURRENT, USE_CONFLICTING, KEEP_BOTH }

data class WebDavSyncProvider(
    val serverUrl: String,
    val remoteRoot: String,
    val username: String,
    val password: String,
)

data class SyncError(val code: String, val message: String, val retryable: Boolean)

data class SyncStatus(
    val state: SyncLifecycleState,
    val providerId: String?,
    val vaultId: String?,
    val preset: SyncPreset?,
    val lastSuccessAtEpochMillis: Long?,
    val pendingOperationCount: Long,
    val conflictCount: Long,
    val nextRetryAtEpochMillis: Long?,
    val lastError: SyncError?,
)

data class SyncCreateResult(
    val vaultId: String,
    val deviceId: String,
    val recoveryKey: String?,
    val status: SyncStatus,
)
data class DiscoveredVaultSummary(
    val vaultId: String,
    val preset: SyncPreset,
)

data class SyncPairingInfo(
    val providerId: String,
    val vaultId: String,
    val serverUrl: String?,
    val remoteRoot: String?,
    val username: String?,
)

data class SyncPairingPayload(
    val v: Int = 1,
    val serverUrl: String,
    val remoteRoot: String,
    val username: String,
    val vaultId: String,
    val providerPassword: String? = null,
)

enum class WellKnownSyncProviderId {
    NUTSTORE,
    NEXTCLOUD,
    INFINICLOUD,
    SYNOLOGY,
    ALIST,
    CUSTOM,
}

data class SyncProviderPreset(
    val id: WellKnownSyncProviderId,
    val defaultName: String,
    val defaultServerUrl: String,
    val defaultRemoteRoot: String,
    val usernamePlaceholder: String,
    val helpText: String,
    val authDocUrl: String? = null,
)

val SYNC_PROVIDER_PRESETS: List<SyncProviderPreset> = listOf(
    SyncProviderPreset(
        id = WellKnownSyncProviderId.NUTSTORE,
        defaultName = "Nutstore",
        defaultServerUrl = "https://dav.jianguoyun.com/dav/",
        defaultRemoteRoot = "Sona",
        usernamePlaceholder = "account@example.com",
        helpText = "Generate an app password in Nutstore: Account Info -> Security -> Third-party apps.",
        authDocUrl = "https://help.jianguoyun.com/?p=2064",
    ),
    SyncProviderPreset(
        id = WellKnownSyncProviderId.NEXTCLOUD,
        defaultName = "Nextcloud",
        defaultServerUrl = "https://cloud.example.com/remote.php/dav/files/USERNAME/",
        defaultRemoteRoot = "Sona",
        usernamePlaceholder = "username",
        helpText = "Generate an app password in Nextcloud: Personal Settings -> Security -> Devices & sessions.",
        authDocUrl = "https://docs.nextcloud.com/server/latest/user_manual/en/session_management.html#device-passwords",
    ),
    SyncProviderPreset(
        id = WellKnownSyncProviderId.INFINICLOUD,
        defaultName = "InfiniCloud",
        defaultServerUrl = "https://teracloud.jp/dav/",
        defaultRemoteRoot = "Sona",
        usernamePlaceholder = "username",
        helpText = "Enable WebDAV connection in InfiniCloud account settings and use the generated connection password.",
        authDocUrl = "https://infinicloud.com/en/support.html",
    ),
    SyncProviderPreset(
        id = WellKnownSyncProviderId.SYNOLOGY,
        defaultName = "Synology NAS",
        defaultServerUrl = "https://nas.example.com:5006/home/",
        defaultRemoteRoot = "Sona",
        usernamePlaceholder = "dsm_user",
        helpText = "Enable WebDAV Server package in DSM with HTTPS port 5006.",
    ),
    SyncProviderPreset(
        id = WellKnownSyncProviderId.ALIST,
        defaultName = "Alist",
        defaultServerUrl = "https://alist.example.com/dav/",
        defaultRemoteRoot = "Sona",
        usernamePlaceholder = "admin",
        helpText = "Use the WebDAV path configured in Alist with account credentials.",
    ),
    SyncProviderPreset(
        id = WellKnownSyncProviderId.CUSTOM,
        defaultName = "Custom WebDAV",
        defaultServerUrl = "https://",
        defaultRemoteRoot = "Sona",
        usernamePlaceholder = "username",
        helpText = "Standard WebDAV server with HTTPS and Digest/Basic authentication.",
    ),
)

fun detectProviderPresetId(serverUrl: String): WellKnownSyncProviderId {
    val normalized = serverUrl.trim().lowercase()
    return when {
        "jianguoyun.com" in normalized -> WellKnownSyncProviderId.NUTSTORE
        "teracloud.jp" in normalized || "infinicloud" in normalized -> WellKnownSyncProviderId.INFINICLOUD
        "remote.php/dav" in normalized || "nextcloud" in normalized || "owncloud" in normalized -> WellKnownSyncProviderId.NEXTCLOUD
        ":5006" in normalized || "synology" in normalized -> WellKnownSyncProviderId.SYNOLOGY
        "/dav" in normalized && "alist" in normalized -> WellKnownSyncProviderId.ALIST
        else -> WellKnownSyncProviderId.CUSTOM
    }
}

fun encodeSyncPairingToken(
    serverUrl: String,
    remoteRoot: String,
    username: String,
    vaultId: String,
    providerPassword: String? = null,
): String {
    val escapedServer = escapeJson(serverUrl.trim())
    val escapedRoot = escapeJson(remoteRoot.trim())
    val escapedUser = escapeJson(username.trim())
    val escapedVault = escapeJson(vaultId.trim())
    val passwordField = if (!providerPassword.isNullOrEmpty()) {
        ",\"providerPassword\":\"${escapeJson(providerPassword)}\""
    } else ""
    val json = """{"v":1,"serverUrl":"$escapedServer","remoteRoot":"$escapedRoot","username":"$escapedUser","vaultId":"$escapedVault"$passwordField}"""
    val base64 = java.util.Base64.getEncoder().encodeToString(json.toByteArray(Charsets.UTF_8))
    val encodedData = java.net.URLEncoder.encode(base64, "UTF-8")
    return "sonasync://v1?data=$encodedData"
}

fun decodeSyncPairingToken(token: String): SyncPairingPayload? {
    val trimmed = token.trim()
    val base64 = if (trimmed.startsWith("sonasync://", ignoreCase = true)) {
        val queryStart = trimmed.indexOf('?')
        if (queryStart == -1) return null
        val query = trimmed.substring(queryStart + 1)
        val params = query.split('&')
        val dataParam = params.firstOrNull { it.startsWith("data=") } ?: return null
        val rawData = dataParam.substring(5)
        try {
            java.net.URLDecoder.decode(rawData, "UTF-8")
        } catch (_: Exception) {
            rawData
        }
    } else {
        trimmed
    }
    val json = try {
        val bytes = java.util.Base64.getDecoder().decode(base64)
        String(bytes, Charsets.UTF_8)
    } catch (_: Exception) {
        return null
    }

    val v = extractJsonInt(json, "v") ?: return null
    if (v != 1) return null
    val serverUrl = extractJsonString(json, "serverUrl") ?: return null
    val remoteRoot = extractJsonString(json, "remoteRoot") ?: return null
    val username = extractJsonString(json, "username") ?: return null
    val vaultId = extractJsonString(json, "vaultId") ?: return null
    val providerPassword = extractJsonString(json, "providerPassword")

    return SyncPairingPayload(
        v = v,
        serverUrl = serverUrl,
        remoteRoot = remoteRoot,
        username = username,
        vaultId = vaultId,
        providerPassword = providerPassword,
    )
}

private fun escapeJson(value: String): String {
    val sb = StringBuilder(value.length)
    for (ch in value) {
        when (ch) {
            '\\' -> sb.append("\\\\")
            '"' -> sb.append("\\\"")
            '\b' -> sb.append("\\b")
            '\u000C' -> sb.append("\\f")
            '\n' -> sb.append("\\n")
            '\r' -> sb.append("\\r")
            '\t' -> sb.append("\\t")
            else -> if (ch.code < 0x20) {
                sb.append(String.format("\\u%04x", ch.code))
            } else {
                sb.append(ch)
            }
        }
    }
    return sb.toString()
}

private fun extractJsonString(json: String, key: String): String? {
    val pattern = Regex(""""$key"\s*:\s*"((?:[^"\\]|\\.)*)"""")
    val match = pattern.find(json) ?: return null
    val raw = match.groupValues[1]
    val sb = StringBuilder(raw.length)
    var i = 0
    while (i < raw.length) {
        val ch = raw[i]
        if (ch == '\\' && i + 1 < raw.length) {
            i++
            when (val esc = raw[i]) {
                '"' -> sb.append('"')
                '\\' -> sb.append('\\')
                '/' -> sb.append('/')
                'b' -> sb.append('\b')
                'f' -> sb.append('\u000C')
                'n' -> sb.append('\n')
                'r' -> sb.append('\r')
                't' -> sb.append('\t')
                'u' -> {
                    if (i + 4 < raw.length) {
                        val hex = raw.substring(i + 1, i + 5)
                        hex.toIntOrNull(16)?.let { sb.append(it.toChar()) }
                        i += 4
                    }
                }
                else -> sb.append(esc)
            }
        } else {
            sb.append(ch)
        }
        i++
    }
    return sb.toString()
}

private fun extractJsonInt(json: String, key: String): Int? {
    val pattern = Regex(""""$key"\s*:\s*(-?\d+)""")
    val match = pattern.find(json) ?: return null
    return match.groupValues[1].toIntOrNull()
}

data class SyncJoinPreview(
    val localOperationCount: Long,
    val remoteOperationCount: Long,
    val projectedConflictCount: Long,
)

data class SyncRunResult(
    val pulledSegmentCount: Long,
    val pushedSegmentCount: Long,
    val appliedOperationCount: Long,
    val publishedOperationCount: Long,
    val conflictCount: Long,
)

data class SyncConflict(
    val id: String,
    val kind: String,
    val entityKind: String,
    val entityId: String,
    val field: String?,
    val createdAtEpochMillis: Long,
)

data class SyncOperation(
    val id: String,
    val sourceDeviceId: String,
    val sourceSequence: Long,
    val entityKind: String,
    val entityId: String,
    val kind: String,
)

data class SyncConflictDetail(
    val summary: SyncConflict,
    val current: SyncOperation,
    val conflicting: SyncOperation,
)

interface SyncPort {
    suspend fun testProvider(provider: WebDavSyncProvider): String
    suspend fun status(): SyncStatus
    suspend fun discoverVaults(provider: WebDavSyncProvider): List<DiscoveredVaultSummary>
    suspend fun getPairingInfo(): SyncPairingInfo?
    suspend fun createVault(
        provider: WebDavSyncProvider,
        preset: SyncPreset,
        masterPassword: String,
        vaultId: String? = null,
        createRecoveryKey: Boolean = true,
    ): SyncCreateResult
    suspend fun previewJoin(provider: WebDavSyncProvider, vaultId: String, masterPassword: String): SyncJoinPreview
    suspend fun join(provider: WebDavSyncProvider, vaultId: String, masterPassword: String): SyncRunResult
    suspend fun unlock(providerPassword: String, masterPassword: String): SyncStatus
    suspend fun unlockWithRecovery(providerPassword: String, recoveryKey: String): SyncStatus
    suspend fun lock()
    suspend fun setPaused(paused: Boolean): SyncStatus
    suspend fun disconnect(): SyncStatus
    suspend fun runNow(): SyncRunResult
    suspend fun changePreset(preset: SyncPreset, confirmShrink: Boolean): SyncStatus
    suspend fun changeMasterPassword(currentPassword: String, nextPassword: String)
    suspend fun generateRecoveryKey(): String
    suspend fun listConflicts(): List<SyncConflict>
    suspend fun conflictDetail(conflictId: String): SyncConflictDetail?
    suspend fun resolveConflict(conflictId: String, resolution: SyncConflictResolution)
}

interface SyncSchedulerPort {
    fun schedulePeriodic()
    fun scheduleAfterLocalChange()
    fun scheduleImmediate()
    fun cancelAll()
}
