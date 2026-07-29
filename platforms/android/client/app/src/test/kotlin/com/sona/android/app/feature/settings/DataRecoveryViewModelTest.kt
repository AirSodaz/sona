package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.data.BackupApplyResult
import com.sona.android.application.data.BackupCounts
import com.sona.android.application.data.BackupManifest
import com.sona.android.application.data.BackupPort
import com.sona.android.application.data.BackupScopes
import com.sona.android.application.data.DataTransferBlocker
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.data.PreparedBackupImport
import com.sona.android.application.recovery.RecoveryControllerPort
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.sync.SyncSchedulerPort
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class DataRecoveryViewModelTest {
    @get:Rule val dispatcherRule = MainDispatcherRule()

    @Test
    fun `backup restore requires inspect and rebinds after destructive import`() = runTest {
        val backup = FakeBackup()
        val files = FakeFiles()
        val recovery = FakeRecovery()
        val scheduler = FakeScheduler()
        var rebound = 0
        val viewModel = DataRecoveryViewModel(backup, files, recovery, scheduler, "1.0", { rebound += 1 })

        viewModel.inspectBackup("content://backup")
        advanceUntilIdle()
        assertEquals(manifest, viewModel.state.value.preparedBackup?.manifest)

        viewModel.confirmImport()
        advanceUntilIdle()

        assertEquals(listOf(true), backup.confirmations)
        assertEquals(1, scheduler.cancelled)
        assertEquals(1, rebound)
        assertEquals(listOf("/cache/import.tar.bz2"), files.cleaned)
        assertEquals(1, viewModel.state.value.restoreGeneration)
        assertNull(viewModel.state.value.preparedBackup)
    }

    @Test
    fun `backup blocker prevents staging or database changes`() = runTest {
        val backup = FakeBackup()
        val files = FakeFiles()
        val viewModel = DataRecoveryViewModel(backup, files, FakeRecovery(), FakeScheduler(), "1.0", {})
        viewModel.setBlockers(setOf(DataTransferBlocker.SYNC))

        viewModel.inspectBackup("content://backup")
        advanceUntilIdle()

        assertEquals(0, files.stageCalls)
        assertTrue(backup.confirmations.isEmpty())
    }

    @Test
    fun `inspect failure cleans staging and cannot enable import`() = runTest {
        val backup = FakeBackup().apply { inspectFailure = IllegalArgumentException("private archive path") }
        val files = FakeFiles()
        val viewModel = DataRecoveryViewModel(backup, files, FakeRecovery(), FakeScheduler(), "1.0", {})

        viewModel.inspectBackup("content://backup")
        advanceUntilIdle()

        assertEquals(listOf("/cache/import.tar.bz2"), files.cleaned)
        assertNull(viewModel.state.value.preparedBackup)
        assertEquals("Data operation failed.", viewModel.state.value.error)
        assertFalse(viewModel.state.value.toString().contains("private archive path"))
    }

    private class FakeBackup : BackupPort {
        val confirmations = mutableListOf<Boolean>()
        var inspectFailure: Exception? = null
        override suspend fun exportBackup(archivePath: String, appVersion: String) = manifest
        override suspend fun inspectBackup(archivePath: String): PreparedBackupImport {
            inspectFailure?.let { throw it }
            return PreparedBackupImport("import-1", archivePath, manifest)
        }
        override suspend fun importBackup(archivePath: String, defaultRuleSetName: String, confirmReplace: Boolean): BackupApplyResult {
            confirmations += confirmReplace
            return BackupApplyResult("import-1", manifest)
        }
        override fun releaseApplicationContext() = true
    }

    private class FakeFiles : FileTransferPort {
        var stageCalls = 0
        val cleaned = mutableListOf<String>()
        override suspend fun stageImport(sourceUri: String): String {
            stageCalls += 1
            return "/cache/import.tar.bz2"
        }
        override suspend fun publishExport(stagedPath: String, destinationUri: String) = Unit
        override suspend fun createExportStagingPath(fileName: String) = "/cache/export.tar.bz2"
        override suspend fun cleanup(path: String) { cleaned += path }
        override suspend fun publishText(text: String, destinationUri: String) = Unit
    }

    private class FakeRecovery : RecoveryControllerPort {
        override val state = MutableStateFlow(RecoverySnapshot(1, null, emptyList()))
        override suspend fun refresh() = Unit
        override suspend fun resume(itemId: String) = Unit
        override suspend fun resumeAll() = Unit
        override suspend fun discard(itemId: String) = Unit
        override suspend fun clearResolved() = Unit
    }

    private class FakeScheduler : SyncSchedulerPort {
        var cancelled = 0
        override fun schedulePeriodic() = Unit
        override fun scheduleAfterLocalChange() = Unit
        override fun scheduleImmediate() = Unit
        override fun cancelAll() { cancelled += 1 }
    }

    companion object {
        private val manifest = BackupManifest(
            1,
            "2026-07-29T00:00:00Z",
            "1.0",
            "lightweight",
            BackupScopes(true, true, true, true, true),
            BackupCounts(2, 3, 4, 0, 0, 0, 0, 0),
        )
    }
}
