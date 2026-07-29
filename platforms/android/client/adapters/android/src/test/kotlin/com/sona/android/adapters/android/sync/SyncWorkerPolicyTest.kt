package com.sona.android.adapters.android.sync

import androidx.work.NetworkType
import com.sona.android.application.sync.SyncConflict
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncCreateResult
import com.sona.android.application.sync.SyncError
import com.sona.android.application.sync.SyncJoinPreview
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.SyncRunResult
import com.sona.android.application.sync.SyncStatus
import com.sona.android.application.sync.WebDavSyncProvider
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

class SyncWorkerPolicyTest {
    @Test
    fun `worker no-ops for non-runnable lifecycle states`() = runTest {
        listOf(
            SyncLifecycleState.DISABLED,
            SyncLifecycleState.LOCKED,
            SyncLifecycleState.PAUSED,
            SyncLifecycleState.SYNCING,
        ).forEach { state ->
            val sync = FakeSyncPort(status(state))
            assertEquals(SyncWorkOutcome.SUCCESS, runSyncWork(sync))
            assertEquals(0, sync.runCount)
        }
    }

    @Test
    fun `worker runs idle sync once`() = runTest {
        val sync = FakeSyncPort(status(SyncLifecycleState.IDLE))

        assertEquals(SyncWorkOutcome.SUCCESS, runSyncWork(sync))
        assertEquals(1, sync.runCount)
    }

    @Test
    fun `only retryable structured errors retry`() = runTest {
        val retryable = FakeSyncPort(status(SyncLifecycleState.ERROR, retryable = true))
        val terminal = FakeSyncPort(status(SyncLifecycleState.ERROR, retryable = false))

        assertEquals(SyncWorkOutcome.RETRY, runSyncWork(retryable))
        assertEquals(SyncWorkOutcome.FAILURE, runSyncWork(terminal))
        assertEquals(0, retryable.runCount)
        assertEquals(0, terminal.runCount)
    }

    @Test
    fun `sync work requires a connected network`() {
        assertEquals(NetworkType.CONNECTED, syncNetworkConstraints().requiredNetworkType)
    }

    private fun status(state: SyncLifecycleState, retryable: Boolean = false) = SyncStatus(
        state = state,
        providerId = null,
        vaultId = null,
        preset = null,
        lastSuccessAtEpochMillis = null,
        pendingOperationCount = 0,
        conflictCount = 0,
        nextRetryAtEpochMillis = null,
        lastError = if (state == SyncLifecycleState.ERROR) SyncError("sync", "failed", retryable) else null,
    )

    private class FakeSyncPort(private var current: SyncStatus) : SyncPort {
        var runCount = 0
        override suspend fun status() = current
        override suspend fun runNow(): SyncRunResult {
            runCount += 1
            return SyncRunResult(0, 0, 0, 0, 0)
        }
        override suspend fun testProvider(provider: WebDavSyncProvider) = "WebDAV"
        override suspend fun createVault(provider: WebDavSyncProvider, preset: SyncPreset, masterPassword: String): SyncCreateResult = error("unused")
        override suspend fun previewJoin(provider: WebDavSyncProvider, vaultId: String, masterPassword: String): SyncJoinPreview = error("unused")
        override suspend fun join(provider: WebDavSyncProvider, vaultId: String, masterPassword: String): SyncRunResult = error("unused")
        override suspend fun unlock(providerPassword: String, masterPassword: String) = current
        override suspend fun unlockWithRecovery(providerPassword: String, recoveryKey: String) = current
        override suspend fun lock() = Unit
        override suspend fun setPaused(paused: Boolean) = current
        override suspend fun disconnect() = current
        override suspend fun changePreset(preset: SyncPreset, confirmShrink: Boolean) = current
        override suspend fun changeMasterPassword(currentPassword: String, nextPassword: String) = Unit
        override suspend fun generateRecoveryKey() = "key"
        override suspend fun listConflicts() = emptyList<SyncConflict>()
        override suspend fun conflictDetail(conflictId: String) = null
        override suspend fun resolveConflict(conflictId: String, resolution: SyncConflictResolution) = Unit
    }
}
