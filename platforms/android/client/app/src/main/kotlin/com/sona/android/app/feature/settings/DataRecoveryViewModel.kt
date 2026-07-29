package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.data.BackupManifest
import com.sona.android.application.data.BackupPort
import com.sona.android.application.data.DataTransferBlocker
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.data.PreparedBackupImport
import com.sona.android.application.recovery.RecoveryControllerPort
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.sync.SyncSchedulerPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class DataRecoveryUiState(
    val recovery: RecoverySnapshot = RecoverySnapshot(1, null, emptyList()),
    val preparedBackup: PreparedBackupImport? = null,
    val exportedManifest: BackupManifest? = null,
    val blockers: Set<DataTransferBlocker> = emptySet(),
    val busy: Boolean = false,
    val restoreGeneration: Int = 0,
    val error: String? = null,
)

class DataRecoveryViewModel(
    private val backups: BackupPort,
    private val files: FileTransferPort,
    private val recovery: RecoveryControllerPort,
    private val syncScheduler: SyncSchedulerPort,
    private val appVersion: String,
    private val onContextReleased: () -> Unit,
) : ViewModel() {
    private val mutableState = MutableStateFlow(DataRecoveryUiState())
    val state: StateFlow<DataRecoveryUiState> = mutableState.asStateFlow()
    private var preparedPath: String? = null

    init {
        viewModelScope.launch { recovery.state.collect { snapshot ->
            mutableState.update { it.copy(recovery = snapshot) }
        } }
    }

    fun setBlockers(blockers: Set<DataTransferBlocker>) = mutableState.update { it.copy(blockers = blockers) }
    fun refreshRecovery() = launchAction { recovery.refresh() }
    fun resume(id: String) = launchAction { recovery.resume(id) }
    fun resumeAll() = launchAction { recovery.resumeAll() }
    fun discard(id: String) = launchAction { recovery.discard(id) }
    fun clearResolved() = launchAction { recovery.clearResolved() }

    fun exportBackup(destinationUri: String) = launchAction(requireIdle = true) {
        val path = files.createExportStagingPath("sona-backup.tar.bz2")
        try {
            val manifest = backups.exportBackup(path, appVersion)
            files.publishExport(path, destinationUri)
            mutableState.update { it.copy(exportedManifest = manifest) }
        } finally {
            files.cleanup(path)
        }
    }

    fun inspectBackup(sourceUri: String) = launchAction(requireIdle = true) {
        preparedPath?.let { files.cleanup(it) }
        val path = files.stageImport(sourceUri)
        try {
            val prepared = backups.inspectBackup(path)
            preparedPath = path
            mutableState.update { it.copy(preparedBackup = prepared) }
        } catch (error: Throwable) {
            files.cleanup(path)
            throw error
        }
    }

    fun cancelPreparedBackup() = launchAction {
        preparedPath?.let { files.cleanup(it) }
        preparedPath = null
        mutableState.update { it.copy(preparedBackup = null) }
    }

    fun confirmImport() = launchAction(requireIdle = true) {
        val prepared = checkNotNull(mutableState.value.preparedBackup)
        val path = checkNotNull(preparedPath)
        var rebound = false
        try {
            syncScheduler.cancelAll()
            backups.importBackup(path, "Default", confirmReplace = true)
            check(backups.releaseApplicationContext()) { "Application context could not be released." }
            onContextReleased()
            rebound = true
            recovery.refresh()
            mutableState.update {
                it.copy(preparedBackup = null, restoreGeneration = it.restoreGeneration + 1)
            }
        } finally {
            if (!rebound) syncScheduler.schedulePeriodic()
            files.cleanup(path)
            preparedPath = null
        }
    }

    private fun launchAction(requireIdle: Boolean = false, block: suspend () -> Unit) {
        if (mutableState.value.busy) return
        if (requireIdle && mutableState.value.blockers.isNotEmpty()) {
            mutableState.update { it.copy(error = "Stop active work before using backups.") }
            return
        }
        mutableState.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try { block() }
            catch (error: CancellationException) { throw error }
            catch (_: Exception) { mutableState.update { it.copy(error = "Data operation failed.") } }
            finally { mutableState.update { it.copy(busy = false) } }
        }
    }

    companion object {
        fun factory(
            backups: BackupPort,
            files: FileTransferPort,
            recovery: RecoveryControllerPort,
            syncScheduler: SyncSchedulerPort,
            appVersion: String,
            onContextReleased: () -> Unit,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = DataRecoveryViewModel(
                backups, files, recovery, syncScheduler, appVersion, onContextReleased,
            ) as T
        }
    }
}
