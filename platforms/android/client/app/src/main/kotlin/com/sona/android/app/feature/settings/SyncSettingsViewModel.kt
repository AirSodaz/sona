package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.sync.DiscoveredVaultSummary
import com.sona.android.application.sync.SyncConflict
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncConflictDetail
import com.sona.android.application.sync.SyncJoinPreview
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPairingInfo
import com.sona.android.application.sync.SyncPairingPayload
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.SyncSchedulerPort
import com.sona.android.application.sync.SyncStatus
import com.sona.android.application.sync.WebDavSyncProvider
import com.sona.android.application.sync.decodeSyncPairingToken
import com.sona.android.application.sync.encodeSyncPairingToken
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
    val pairingInfo: SyncPairingInfo? = null,
    val discoveredVaults: List<DiscoveredVaultSummary>? = null,
    val pairingSuccessNotice: String? = null,
    val busy: Boolean = false,
    val busyAction: String? = null,
    val error: String? = null,
)

class SyncSettingsViewModel(
    private val sync: SyncPort,
    private val scheduler: SyncSchedulerPort,
    private val files: FileTransferPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(SyncSettingsUiState())
    val state: StateFlow<SyncSettingsUiState> = mutableState.asStateFlow()

    fun refresh() = launchAction("refresh") {
        val status = sync.status()
        val pairing = if (status.state != SyncLifecycleState.DISABLED) sync.getPairingInfo() else null
        mutableState.update { it.copy(status = status, conflicts = sync.listConflicts(), pairingInfo = pairing) }
        if (status.state !in setOf(SyncLifecycleState.DISABLED, SyncLifecycleState.PAUSED)) {
            scheduler.schedulePeriodic()
        }
    }

    fun testProvider(provider: WebDavSyncProvider) = launchAction { sync.testProvider(provider) }

    fun create(
        provider: WebDavSyncProvider,
        preset: SyncPreset,
        password: String,
        vaultId: String? = null,
        createRecoveryKey: Boolean = true,
    ) = launchAction("create") {
        val result = sync.createVault(provider, preset, password, vaultId, createRecoveryKey)
        scheduler.schedulePeriodic()
        val pairing = sync.getPairingInfo()
        mutableState.update { it.copy(status = result.status, recoveryKey = result.recoveryKey, pairingInfo = pairing) }
    }

    fun discoverVaults(provider: WebDavSyncProvider, onResult: ((List<DiscoveredVaultSummary>) -> Unit)? = null) = launchAction("discover") {
        val vaults = sync.discoverVaults(provider)
        mutableState.update { it.copy(discoveredVaults = vaults) }
        onResult?.invoke(vaults)
    }

    fun clearDiscoveredVaults() = mutableState.update { it.copy(discoveredVaults = null) }

    fun applyPairingToken(token: String): SyncPairingPayload? {
        val payload = decodeSyncPairingToken(token)
        if (payload == null) {
            mutableState.update { it.copy(error = "Invalid pairing token format.") }
            return null
        }
        mutableState.update {
            it.copy(
                error = null,
                pairingSuccessNotice = "Imported connection parameters (Vault: ${payload.vaultId})",
            )
        }
        return payload
    }

    fun clearPairingNotice() = mutableState.update { it.copy(pairingSuccessNotice = null) }

    fun getPairingToken(includePassword: Boolean = false, password: String = ""): String? {
        val info = mutableState.value.pairingInfo ?: return null
        val serverUrl = info.serverUrl ?: return null
        val remoteRoot = info.remoteRoot ?: "Sona"
        val username = info.username ?: ""
        return encodeSyncPairingToken(
            serverUrl = serverUrl,
            remoteRoot = remoteRoot,
            username = username,
            vaultId = info.vaultId,
            providerPassword = if (includePassword && password.isNotEmpty()) password else null,
        )
    }

    fun previewJoin(provider: WebDavSyncProvider, vaultId: String, password: String) = launchAction {
        mutableState.update { it.copy(joinPreview = sync.previewJoin(provider, vaultId, password)) }
    }

    fun join(provider: WebDavSyncProvider, vaultId: String, password: String) = launchAction {
        sync.join(provider, vaultId, password)
        scheduler.schedulePeriodic()
        refreshState()
    }

    fun unlock(providerPassword: String, masterPassword: String) = launchAction("unlock") {
        val status = sync.unlock(providerPassword, masterPassword)
        val pairing = sync.getPairingInfo()
        mutableState.update { it.copy(status = status, pairingInfo = pairing) }
        scheduler.schedulePeriodic()
    }

    fun unlockWithRecovery(providerPassword: String, recoveryKey: String) = launchAction("unlockWithRecovery") {
        val status = sync.unlockWithRecovery(providerPassword, recoveryKey)
        val pairing = sync.getPairingInfo()
        mutableState.update { it.copy(status = status, pairingInfo = pairing) }
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

    fun disconnect() = launchAction("disconnect") {
        mutableState.update { it.copy(status = sync.disconnect(), conflicts = emptyList(), pairingInfo = null) }
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
        val status = sync.status()
        val pairing = if (status.state != SyncLifecycleState.DISABLED) sync.getPairingInfo() else null
        mutableState.update { it.copy(status = status, conflicts = sync.listConflicts(), pairingInfo = pairing) }
    }

    private fun launchAction(action: String? = null, block: suspend () -> Unit) {
        if (mutableState.value.busy) return
        mutableState.update { it.copy(busy = true, busyAction = action, error = null) }
        viewModelScope.launch {
            try {
                block()
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update { it.copy(error = "Sync operation failed.") }
            } finally {
                mutableState.update { it.copy(busy = false, busyAction = null) }
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
