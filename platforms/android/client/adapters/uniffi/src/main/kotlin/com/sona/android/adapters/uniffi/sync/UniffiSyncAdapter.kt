package com.sona.android.adapters.uniffi.sync

import com.sona.android.application.sync.SyncConflict
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncConflictDetail
import com.sona.android.application.sync.SyncOperation
import com.sona.android.application.sync.SyncCreateResult
import com.sona.android.application.sync.SyncError
import com.sona.android.application.sync.SyncJoinPreview
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.SyncRunResult
import com.sona.android.application.sync.SyncStatus
import com.sona.android.application.sync.WebDavSyncProvider
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import uniffi.sona_uniffi_bind.FfiSecret
import uniffi.sona_uniffi_bind.FfiSyncChangePasswordRequestV1
import uniffi.sona_uniffi_bind.FfiSyncConflictResolutionV1
import uniffi.sona_uniffi_bind.FfiSyncCreateRequestV1
import uniffi.sona_uniffi_bind.FfiSyncJoinRequestV1
import uniffi.sona_uniffi_bind.FfiSyncLifecycleStateV1
import uniffi.sona_uniffi_bind.FfiSyncOperationKindV1
import uniffi.sona_uniffi_bind.FfiSyncPresetV1
import uniffi.sona_uniffi_bind.FfiSyncProviderInputV1
import uniffi.sona_uniffi_bind.FfiSyncRunResultV1
import uniffi.sona_uniffi_bind.FfiSyncStatusSnapshotV1
import uniffi.sona_uniffi_bind.FfiSyncUnlockRequestV1
import uniffi.sona_uniffi_bind.syncChangeMasterPasswordV1
import uniffi.sona_uniffi_bind.syncChangePresetV1
import uniffi.sona_uniffi_bind.syncCreateVaultV1
import uniffi.sona_uniffi_bind.syncDisconnectV1
import uniffi.sona_uniffi_bind.syncGenerateRecoveryKey
import uniffi.sona_uniffi_bind.syncGetConflictV1
import uniffi.sona_uniffi_bind.syncGetStatusV1
import uniffi.sona_uniffi_bind.syncJoinVaultV1
import uniffi.sona_uniffi_bind.syncListConflictsV1
import uniffi.sona_uniffi_bind.syncLock
import uniffi.sona_uniffi_bind.syncPreviewJoinV1
import uniffi.sona_uniffi_bind.syncResolveConflictV1
import uniffi.sona_uniffi_bind.syncRunNowV1
import uniffi.sona_uniffi_bind.syncSetPausedV1
import uniffi.sona_uniffi_bind.syncTestProviderV1
import uniffi.sona_uniffi_bind.syncUnlockV1
import uniffi.sona_uniffi_bind.syncUnlockWithRecoveryV1

class UniffiSyncAdapter(private val appDataDir: String) : SyncPort {
    init { require(appDataDir.isNotBlank()) { "Sync app data directory must not be blank." } }
    override suspend fun testProvider(provider: WebDavSyncProvider): String =
        syncTestProviderV1(provider.toFfi()).displayName

    override suspend fun status(): SyncStatus = syncGetStatusV1(appDataDir).toApplication()

    override suspend fun createVault(
        provider: WebDavSyncProvider,
        preset: SyncPreset,
        masterPassword: String,
    ): SyncCreateResult {
        val request = FfiSyncCreateRequestV1(
            provider = provider.toFfi(),
            preset = preset.toFfi(),
            masterPassword = FfiSecret(masterPassword),
            createRecoveryKey = false,
        )
        return try {
            val created = syncCreateVaultV1(appDataDir, request)
            val recoveryKey = syncGenerateRecoveryKey(appDataDir)
            SyncCreateResult(created.vaultId, created.deviceId, recoveryKey, created.status.toApplication())
        } finally {
            request.destroy()
        }
    }

    override suspend fun previewJoin(
        provider: WebDavSyncProvider,
        vaultId: String,
        masterPassword: String,
    ): SyncJoinPreview = joinRequest(provider, vaultId, masterPassword) { request ->
        syncPreviewJoinV1(appDataDir, request).let {
            SyncJoinPreview(
                it.localOperationCount.toLongChecked("Local operation count"),
                it.remoteOperationCount.toLongChecked("Remote operation count"),
                it.projectedConflictCount.toLongChecked("Projected conflict count"),
            )
        }
    }

    override suspend fun join(
        provider: WebDavSyncProvider,
        vaultId: String,
        masterPassword: String,
    ): SyncRunResult = joinRequest(provider, vaultId, masterPassword) {
        syncJoinVaultV1(appDataDir, it).toApplication()
    }

    override suspend fun unlock(providerPassword: String, masterPassword: String): SyncStatus =
        unlockRequest(providerPassword, masterPassword, null) {
            syncUnlockV1(appDataDir, it).toApplication()
        }

    override suspend fun unlockWithRecovery(providerPassword: String, recoveryKey: String): SyncStatus =
        unlockRequest(providerPassword, null, recoveryKey) {
            syncUnlockWithRecoveryV1(appDataDir, it).toApplication()
        }

    override suspend fun lock() = syncLock(appDataDir)
    override suspend fun setPaused(paused: Boolean): SyncStatus = syncSetPausedV1(appDataDir, paused).toApplication()
    override suspend fun disconnect(): SyncStatus = syncDisconnectV1(appDataDir).toApplication()
    override suspend fun runNow(): SyncRunResult = syncRunNowV1(appDataDir).toApplication()
    override suspend fun changePreset(preset: SyncPreset, confirmShrink: Boolean): SyncStatus =
        syncChangePresetV1(appDataDir, preset.toFfi(), confirmShrink).toApplication()

    override suspend fun changeMasterPassword(currentPassword: String, nextPassword: String) {
        val request = FfiSyncChangePasswordRequestV1(FfiSecret(currentPassword), FfiSecret(nextPassword))
        try {
            syncChangeMasterPasswordV1(appDataDir, request)
        } finally {
            request.destroy()
        }
    }

    override suspend fun generateRecoveryKey(): String = syncGenerateRecoveryKey(appDataDir)

    override suspend fun listConflicts(): List<SyncConflict> = syncListConflictsV1(appDataDir).map {
        it.toApplication()
    }

    override suspend fun conflictDetail(conflictId: String): SyncConflictDetail? {
        require(conflictId.isNotBlank()) { "Conflict ID must not be blank." }
        return syncGetConflictV1(appDataDir, conflictId)?.let { detail ->
            SyncConflictDetail(
                summary = detail.summary.toApplication(),
                current = SyncOperation(
                    detail.current.operationId,
                    detail.current.sourceDeviceId,
                    detail.current.sourceSequence.toLongChecked("Current source sequence"),
                    detail.current.entity.kind.name,
                    detail.current.entity.id,
                    detail.current.kind.toApplicationLabel(),
                ),
                conflicting = SyncOperation(
                    detail.conflicting.operationId,
                    detail.conflicting.sourceDeviceId,
                    detail.conflicting.sourceSequence.toLongChecked("Conflicting source sequence"),
                    detail.conflicting.entity.kind.name,
                    detail.conflicting.entity.id,
                    detail.conflicting.kind.toApplicationLabel(),
                ),
            )
        }
    }

    override suspend fun resolveConflict(conflictId: String, resolution: SyncConflictResolution) {
        syncResolveConflictV1(
            appDataDir,
            conflictId,
            when (resolution) {
                SyncConflictResolution.KEEP_CURRENT -> FfiSyncConflictResolutionV1.KEEP_CURRENT
                SyncConflictResolution.USE_CONFLICTING -> FfiSyncConflictResolutionV1.USE_CONFLICTING
                SyncConflictResolution.KEEP_BOTH -> FfiSyncConflictResolutionV1.KEEP_BOTH
            },
        )
    }

    private suspend fun <T> joinRequest(
        provider: WebDavSyncProvider,
        vaultId: String,
        masterPassword: String,
        block: suspend (FfiSyncJoinRequestV1) -> T,
    ): T {
        val request = FfiSyncJoinRequestV1(provider.toFfi(), vaultId, FfiSecret(masterPassword))
        return try { block(request) } finally { request.destroy() }
    }

    private suspend fun <T> unlockRequest(
        providerPassword: String,
        masterPassword: String?,
        recoveryKey: String?,
        block: suspend (FfiSyncUnlockRequestV1) -> T,
    ): T {
        val request = FfiSyncUnlockRequestV1(
            FfiSecret(providerPassword),
            masterPassword?.let(::FfiSecret),
            recoveryKey?.let(::FfiSecret),
        )
        return try { block(request) } finally { request.destroy() }
    }
}

internal fun WebDavSyncProvider.toFfi(): FfiSyncProviderInputV1 {
    require(serverUrl.trim().startsWith("https://", ignoreCase = true)) { "WebDAV requires HTTPS." }
    val configuration = buildJsonObject {
        put("serverUrl", serverUrl.trim())
        put("remoteRoot", remoteRoot.trim())
        put("username", username.trim())
        put("password", password)
    }
    return FfiSyncProviderInputV1("webdav", configuration.toString())
}

private fun SyncPreset.toFfi() = when (this) {
    SyncPreset.CONTENT -> FfiSyncPresetV1.CONTENT
    SyncPreset.STANDARD -> FfiSyncPresetV1.STANDARD
    SyncPreset.FULL -> FfiSyncPresetV1.FULL
}

internal fun FfiSyncStatusSnapshotV1.toApplication() = SyncStatus(
    state = when (state) {
        FfiSyncLifecycleStateV1.DISABLED -> SyncLifecycleState.DISABLED
        FfiSyncLifecycleStateV1.LOCKED -> SyncLifecycleState.LOCKED
        FfiSyncLifecycleStateV1.IDLE -> SyncLifecycleState.IDLE
        FfiSyncLifecycleStateV1.SYNCING -> SyncLifecycleState.SYNCING
        FfiSyncLifecycleStateV1.PAUSED -> SyncLifecycleState.PAUSED
        FfiSyncLifecycleStateV1.ERROR -> SyncLifecycleState.ERROR
    },
    providerId = providerId,
    vaultId = vaultId,
    preset = preset?.let { SyncPreset.valueOf(it.name) },
    lastSuccessAtEpochMillis = lastSuccessAtMs?.toLongChecked("Last success timestamp"),
    pendingOperationCount = pendingOperationCount.toLongChecked("Pending operation count"),
    conflictCount = conflictCount.toLongChecked("Conflict count"),
    nextRetryAtEpochMillis = nextRetryAtMs?.toLongChecked("Next retry timestamp"),
    lastError = lastError?.let { SyncError(it.code, it.message, it.retryable) },
)

internal fun FfiSyncRunResultV1.toApplication() = SyncRunResult(
    pulledSegmentCount.toLongChecked("Pulled segment count"),
    pushedSegmentCount.toLongChecked("Pushed segment count"),
    appliedOperationCount.toLongChecked("Applied operation count"),
    publishedOperationCount.toLongChecked("Published operation count"),
    conflictCount.toLongChecked("Conflict count"),
)

private fun uniffi.sona_uniffi_bind.FfiSyncConflictSummaryV1.toApplication() = SyncConflict(
    id = conflictId,
    kind = kind.name,
    entityKind = entity.kind.name,
    entityId = entity.id,
    field = field,
    createdAtEpochMillis = createdAtMs.toLongChecked("Conflict timestamp"),
)

private fun FfiSyncOperationKindV1.toApplicationLabel() = when (this) {
    is FfiSyncOperationKindV1.SetField -> "SET_FIELD:$field"
    FfiSyncOperationKindV1.DeleteEntity -> "DELETE_ENTITY"
}

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}
