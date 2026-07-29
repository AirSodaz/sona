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
    suspend fun createVault(provider: WebDavSyncProvider, preset: SyncPreset, masterPassword: String): SyncCreateResult
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
