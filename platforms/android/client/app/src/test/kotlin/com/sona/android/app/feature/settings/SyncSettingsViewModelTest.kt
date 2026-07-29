package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.sync.SyncConflict
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncCreateResult
import com.sona.android.application.sync.SyncJoinPreview
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.SyncRunResult
import com.sona.android.application.sync.SyncSchedulerPort
import com.sona.android.application.sync.SyncStatus
import com.sona.android.application.sync.WebDavSyncProvider
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test

class SyncSettingsViewModelTest {
    @get:Rule val dispatcherRule = MainDispatcherRule()

    @Test
    fun `manual sync is scheduled through unique work`() = runTest {
        val sync = FakeSyncPort()
        val scheduler = FakeScheduler()
        val viewModel = SyncSettingsViewModel(sync, scheduler, FakeFiles)

        viewModel.runNow()
        advanceUntilIdle()

        assertEquals(1, scheduler.immediate)
        assertEquals(0, sync.runCalls)
    }

    @Test
    fun `create schedules periodic work and exposes recovery key once`() = runTest {
        val sync = FakeSyncPort()
        val scheduler = FakeScheduler()
        val viewModel = SyncSettingsViewModel(sync, scheduler, FakeFiles)

        viewModel.create(WebDavSyncProvider("https://dav.example", "Sona", "u", "p"), SyncPreset.STANDARD, "master")
        advanceUntilIdle()

        assertEquals(1, scheduler.periodic)
        assertEquals("recovery-key", viewModel.state.value.recoveryKey)
        viewModel.consumeRecoveryKey()
        assertNull(viewModel.state.value.recoveryKey)
    }

    @Test
    fun `pause cancels work and resume registers the safety net`() = runTest {
        val sync = FakeSyncPort()
        val scheduler = FakeScheduler()
        val viewModel = SyncSettingsViewModel(sync, scheduler, FakeFiles)

        viewModel.setPaused(true)
        advanceUntilIdle()
        viewModel.setPaused(false)
        advanceUntilIdle()

        assertEquals(1, scheduler.cancelled)
        assertEquals(1, scheduler.periodic)
    }

    @Test
    fun `operation errors do not expose provider secrets`() = runTest {
        val sync = FakeSyncPort().apply { failure = IllegalStateException("https://dav.example bearer-secret") }
        val viewModel = SyncSettingsViewModel(sync, FakeScheduler(), FakeFiles)

        viewModel.refresh()
        advanceUntilIdle()

        assertEquals("Sync operation failed.", viewModel.state.value.error)
        assertFalse(viewModel.state.value.toString().contains("bearer-secret"))
    }

    private class FakeScheduler : SyncSchedulerPort {
        var periodic = 0
        var immediate = 0
        var cancelled = 0
        override fun schedulePeriodic() { periodic += 1 }
        override fun scheduleAfterLocalChange() = Unit
        override fun scheduleImmediate() { immediate += 1 }
        override fun cancelAll() { cancelled += 1 }
    }

    private class FakeSyncPort : SyncPort {
        var current = status(SyncLifecycleState.IDLE)
        var runCalls = 0
        var failure: Exception? = null
        override suspend fun status(): SyncStatus = failure?.let { throw it } ?: current
        override suspend fun createVault(provider: WebDavSyncProvider, preset: SyncPreset, masterPassword: String) =
            SyncCreateResult("vault", "device", "recovery-key", current)
        override suspend fun setPaused(paused: Boolean): SyncStatus = status(
            if (paused) SyncLifecycleState.PAUSED else SyncLifecycleState.IDLE,
        ).also { current = it }
        override suspend fun runNow(): SyncRunResult {
            runCalls += 1
            return SyncRunResult(0, 0, 0, 0, 0)
        }
        override suspend fun testProvider(provider: WebDavSyncProvider) = "WebDAV"
        override suspend fun previewJoin(provider: WebDavSyncProvider, vaultId: String, masterPassword: String) = SyncJoinPreview(0, 0, 0)
        override suspend fun join(provider: WebDavSyncProvider, vaultId: String, masterPassword: String) = SyncRunResult(0, 0, 0, 0, 0)
        override suspend fun unlock(providerPassword: String, masterPassword: String) = current
        override suspend fun unlockWithRecovery(providerPassword: String, recoveryKey: String) = current
        override suspend fun lock() = Unit
        override suspend fun disconnect() = status(SyncLifecycleState.DISABLED)
        override suspend fun changePreset(preset: SyncPreset, confirmShrink: Boolean) = current
        override suspend fun changeMasterPassword(currentPassword: String, nextPassword: String) = Unit
        override suspend fun generateRecoveryKey() = "recovery-key"
        override suspend fun listConflicts() = emptyList<SyncConflict>()
        override suspend fun conflictDetail(conflictId: String) = null
        override suspend fun resolveConflict(conflictId: String, resolution: SyncConflictResolution) = Unit
    }

    private object FakeFiles : FileTransferPort {
        override suspend fun stageImport(sourceUri: String) = error("unused")
        override suspend fun publishExport(stagedPath: String, destinationUri: String) = Unit
        override suspend fun createExportStagingPath(fileName: String) = error("unused")
        override suspend fun cleanup(path: String) = Unit
        override suspend fun publishText(text: String, destinationUri: String) = Unit
    }

    companion object {
        private fun status(state: SyncLifecycleState) = SyncStatus(state, null, null, null, null, 0, 0, null, null)
    }
}
