package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.sync.SyncConflict
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncConflictDetail
import com.sona.android.application.sync.SyncJoinPreview
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.SyncSchedulerPort
import com.sona.android.application.sync.SyncStatus
import com.sona.android.application.sync.WebDavSyncProvider
import com.sona.android.application.data.FileTransferPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SyncSettingsUiState(
    val status: SyncStatus = DISABLED_SYNC_STATUS,
    val conflicts: List<SyncConflict> = emptyList(),
    val joinPreview: SyncJoinPreview? = null,
    val recoveryKey: String? = null,
    val conflictDetail: SyncConflictDetail? = null,
    val busy: Boolean = false,
    val error: String? = null,
)

class SyncSettingsViewModel(
    private val sync: SyncPort,
    private val scheduler: SyncSchedulerPort,
    private val files: FileTransferPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(SyncSettingsUiState())
    val state: StateFlow<SyncSettingsUiState> = mutableState.asStateFlow()

    fun refresh() = launchAction {
        val status = sync.status()
        mutableState.update { it.copy(status = status, conflicts = sync.listConflicts()) }
        if (status.state !in setOf(SyncLifecycleState.DISABLED, SyncLifecycleState.PAUSED)) {
            scheduler.schedulePeriodic()
        }
    }

    fun testProvider(provider: WebDavSyncProvider) = launchAction { sync.testProvider(provider) }

    fun create(provider: WebDavSyncProvider, preset: SyncPreset, password: String) = launchAction {
        val result = sync.createVault(provider, preset, password)
        scheduler.schedulePeriodic()
        mutableState.update { it.copy(status = result.status, recoveryKey = result.recoveryKey) }
    }

    fun previewJoin(provider: WebDavSyncProvider, vaultId: String, password: String) = launchAction {
        mutableState.update { it.copy(joinPreview = sync.previewJoin(provider, vaultId, password)) }
    }

    fun join(provider: WebDavSyncProvider, vaultId: String, password: String) = launchAction {
        sync.join(provider, vaultId, password)
        scheduler.schedulePeriodic()
        refreshState()
    }

    fun unlock(providerPassword: String, masterPassword: String) = launchAction {
        mutableState.update { it.copy(status = sync.unlock(providerPassword, masterPassword)) }
        scheduler.schedulePeriodic()
    }

    fun unlockWithRecovery(providerPassword: String, recoveryKey: String) = launchAction {
        mutableState.update { it.copy(status = sync.unlockWithRecovery(providerPassword, recoveryKey)) }
        scheduler.schedulePeriodic()
    }

    fun runNow() = launchAction {
        scheduler.scheduleImmediate()
        refreshState()
    }

    fun setPaused(paused: Boolean) = launchAction {
        val status = sync.setPaused(paused)
        mutableState.update { it.copy(status = status) }
        if (paused) scheduler.cancelAll() else scheduler.schedulePeriodic()
    }

    fun lock() = launchAction {
        sync.lock()
        scheduler.cancelAll()
        refreshState()
    }

    fun disconnect() = launchAction {
        mutableState.update { it.copy(status = sync.disconnect(), conflicts = emptyList()) }
        scheduler.cancelAll()
    }

    fun changePreset(preset: SyncPreset) = launchAction {
        mutableState.update { it.copy(status = sync.changePreset(preset, confirmShrink = true)) }
    }

    fun changePassword(current: String, next: String) = launchAction {
        sync.changeMasterPassword(current, next)
    }

    fun generateRecoveryKey() = launchAction {
        mutableState.update { it.copy(recoveryKey = sync.generateRecoveryKey()) }
    }

    fun consumeRecoveryKey() = mutableState.update { it.copy(recoveryKey = null) }

    fun exportRecoveryKey(destinationUri: String) = launchAction {
        val key = checkNotNull(mutableState.value.recoveryKey)
        files.publishText(key, destinationUri)
        mutableState.update { it.copy(recoveryKey = null) }
    }

    fun resolveConflict(id: String, resolution: SyncConflictResolution) = launchAction {
        sync.resolveConflict(id, resolution)
        mutableState.update { it.copy(conflictDetail = null) }
        refreshState()
    }

    fun loadConflict(id: String) = launchAction {
        mutableState.update { it.copy(conflictDetail = sync.conflictDetail(id)) }
    }

    private suspend fun refreshState() {
        mutableState.update { it.copy(status = sync.status(), conflicts = sync.listConflicts()) }
    }

    private fun launchAction(block: suspend () -> Unit) {
        if (mutableState.value.busy) return
        mutableState.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try {
                block()
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update { it.copy(error = "Sync operation failed.") }
            } finally {
                mutableState.update { it.copy(busy = false) }
            }
        }
    }

    companion object {
        fun factory(sync: SyncPort, scheduler: SyncSchedulerPort, files: FileTransferPort): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T =
                    SyncSettingsViewModel(sync, scheduler, files) as T
            }
    }
}

private val DISABLED_SYNC_STATUS = SyncStatus(
    SyncLifecycleState.DISABLED, null, null, null, null, 0, 0, null, null,
)
