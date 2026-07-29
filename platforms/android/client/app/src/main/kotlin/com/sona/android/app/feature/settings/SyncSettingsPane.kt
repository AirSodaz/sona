package com.sona.android.app.feature.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.WebDavSyncProvider

@Composable
internal fun SyncSettingsPane(
    state: SyncSettingsUiState,
    onRefresh: () -> Unit,
    onTestProvider: (WebDavSyncProvider) -> Unit,
    onCreate: (WebDavSyncProvider, SyncPreset, String) -> Unit,
    onPreviewJoin: (WebDavSyncProvider, String, String) -> Unit,
    onJoin: (WebDavSyncProvider, String, String) -> Unit,
    onUnlock: (String, String) -> Unit,
    onUnlockWithRecovery: (String, String) -> Unit,
    onRunNow: () -> Unit,
    onSetPaused: (Boolean) -> Unit,
    onLock: () -> Unit,
    onDisconnect: () -> Unit,
    onGenerateRecoveryKey: () -> Unit,
    onExportRecoveryKey: (String) -> Unit,
    onConsumeRecoveryKey: () -> Unit,
    onResolveConflict: (String, SyncConflictResolution) -> Unit,
    onLoadConflict: (String) -> Unit,
    onChangePreset: (SyncPreset) -> Unit,
    onChangePassword: (String, String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var server by remember { mutableStateOf("") }
    var root by remember { mutableStateOf("Sona") }
    var username by remember { mutableStateOf("") }
    var providerPassword by remember { mutableStateOf("") }
    var masterPassword by remember { mutableStateOf("") }
    var vaultId by remember { mutableStateOf("") }
    var joinMode by remember { mutableStateOf(false) }
    var recoveryUnlock by remember { mutableStateOf(false) }
    var joinPreviewCurrent by remember { mutableStateOf(false) }
    var currentMasterPassword by remember { mutableStateOf("") }
    var nextMasterPassword by remember { mutableStateOf("") }
    val provider = WebDavSyncProvider(server, root, username, providerPassword)
    val clipboard = LocalClipboardManager.current
    val keyExporter = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("text/plain"),
    ) { uri -> uri?.let { onExportRecoveryKey(it.toString()) } }

    Column(
        modifier = modifier.verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(stringResource(R.string.sync_status, state.status.state.name), style = MaterialTheme.typography.titleMedium)
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }

        when (state.status.state) {
            SyncLifecycleState.DISABLED -> {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(selected = !joinMode, onClick = { joinMode = false; joinPreviewCurrent = false }, label = { Text(stringResource(R.string.sync_create)) })
                    FilterChip(selected = joinMode, onClick = { joinMode = true; joinPreviewCurrent = false }, label = { Text(stringResource(R.string.sync_join)) })
                }
                OutlinedTextField(server, { server = it; joinPreviewCurrent = false }, label = { Text(stringResource(R.string.sync_server_url)) }, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(root, { root = it; joinPreviewCurrent = false }, label = { Text(stringResource(R.string.sync_remote_root)) }, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(username, { username = it; joinPreviewCurrent = false }, label = { Text(stringResource(R.string.sync_username)) }, modifier = Modifier.fillMaxWidth())
                SecretField(providerPassword, { providerPassword = it; joinPreviewCurrent = false }, R.string.sync_provider_password)
                if (joinMode) OutlinedTextField(vaultId, { vaultId = it; joinPreviewCurrent = false }, label = { Text(stringResource(R.string.sync_vault_id)) }, modifier = Modifier.fillMaxWidth())
                SecretField(masterPassword, { masterPassword = it; joinPreviewCurrent = false }, R.string.sync_master_password)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { onTestProvider(provider) }, enabled = !state.busy) { Text(stringResource(R.string.sync_test)) }
                    if (joinMode) {
                        Button(onClick = {
                            joinPreviewCurrent = true
                            onPreviewJoin(provider, vaultId, masterPassword)
                        }, enabled = !state.busy) { Text(stringResource(R.string.sync_preview)) }
                    } else {
                        Button(onClick = { onCreate(provider, SyncPreset.STANDARD, masterPassword) }, enabled = !state.busy) { Text(stringResource(R.string.sync_create)) }
                    }
                }
                state.joinPreview?.takeIf { joinPreviewCurrent }?.let { preview ->
                    Text(stringResource(R.string.sync_preview_counts, preview.localOperationCount, preview.remoteOperationCount, preview.projectedConflictCount))
                    Button(onClick = { onJoin(provider, vaultId, masterPassword) }, enabled = !state.busy) { Text(stringResource(R.string.sync_confirm_join)) }
                }
            }
            SyncLifecycleState.LOCKED -> {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(selected = !recoveryUnlock, onClick = { recoveryUnlock = false }, label = { Text(stringResource(R.string.sync_master_password)) })
                    FilterChip(selected = recoveryUnlock, onClick = { recoveryUnlock = true }, label = { Text(stringResource(R.string.sync_recovery_key)) })
                }
                SecretField(providerPassword, { providerPassword = it }, R.string.sync_provider_password)
                SecretField(masterPassword, { masterPassword = it }, if (recoveryUnlock) R.string.sync_recovery_key else R.string.sync_master_password)
                Button(
                    onClick = { if (recoveryUnlock) onUnlockWithRecovery(providerPassword, masterPassword) else onUnlock(providerPassword, masterPassword) },
                    enabled = !state.busy,
                ) { Text(stringResource(R.string.sync_unlock)) }
            }
            else -> {
                Text(state.status.vaultId.orEmpty(), style = MaterialTheme.typography.bodyMedium)
                Text(stringResource(R.string.sync_counts, state.status.pendingOperationCount, state.status.conflictCount))
                Text(stringResource(R.string.sync_preset), style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SyncPreset.entries.forEach { preset ->
                        FilterChip(
                            selected = state.status.preset == preset,
                            onClick = { onChangePreset(preset) },
                            enabled = !state.busy,
                            label = { Text(preset.name.lowercase()) },
                        )
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = onRunNow, enabled = !state.busy && state.status.state != SyncLifecycleState.PAUSED) { Text(stringResource(R.string.sync_now)) }
                    OutlinedButton(onClick = { onSetPaused(state.status.state != SyncLifecycleState.PAUSED) }, enabled = !state.busy) {
                        Text(stringResource(if (state.status.state == SyncLifecycleState.PAUSED) R.string.sync_resume else R.string.sync_pause))
                    }
                    OutlinedButton(onClick = onLock, enabled = !state.busy) { Text(stringResource(R.string.sync_lock)) }
                }
                OutlinedButton(onClick = onGenerateRecoveryKey, enabled = !state.busy) { Text(stringResource(R.string.sync_generate_recovery_key)) }
                state.recoveryKey?.let { key ->
                    Text(key, style = MaterialTheme.typography.bodySmall)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(onClick = { clipboard.setText(AnnotatedString(key)); onConsumeRecoveryKey() }) { Text(stringResource(R.string.action_copy)) }
                        OutlinedButton(onClick = { keyExporter.launch("sona-recovery-key.txt") }) { Text(stringResource(R.string.action_export)) }
                    }
                }
                SecretField(currentMasterPassword, { currentMasterPassword = it }, R.string.sync_current_master_password)
                SecretField(nextMasterPassword, { nextMasterPassword = it }, R.string.sync_new_master_password)
                OutlinedButton(
                    onClick = {
                        onChangePassword(currentMasterPassword, nextMasterPassword)
                        currentMasterPassword = ""
                        nextMasterPassword = ""
                    },
                    enabled = !state.busy && currentMasterPassword.isNotBlank() && nextMasterPassword.isNotBlank(),
                ) { Text(stringResource(R.string.sync_change_master_password)) }
                HorizontalDivider()
                state.conflicts.forEach { conflict ->
                    TextButton(onClick = { onLoadConflict(conflict.id) }) {
                        Text("${conflict.entityKind} / ${conflict.kind}", style = MaterialTheme.typography.titleSmall)
                    }
                    state.conflictDetail?.takeIf { it.summary.id == conflict.id }?.let { detail ->
                        Text("Current: ${detail.current.kind} · ${detail.current.sourceDeviceId}", style = MaterialTheme.typography.bodySmall)
                        Text("Conflicting: ${detail.conflicting.kind} · ${detail.conflicting.sourceDeviceId}", style = MaterialTheme.typography.bodySmall)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        OutlinedButton(onClick = { onResolveConflict(conflict.id, SyncConflictResolution.KEEP_CURRENT) }) { Text(stringResource(R.string.sync_keep_current)) }
                        OutlinedButton(onClick = { onResolveConflict(conflict.id, SyncConflictResolution.USE_CONFLICTING) }) { Text(stringResource(R.string.sync_use_remote)) }
                        OutlinedButton(onClick = { onResolveConflict(conflict.id, SyncConflictResolution.KEEP_BOTH) }) { Text(stringResource(R.string.sync_keep_both)) }
                    }
                }
                OutlinedButton(onClick = onDisconnect, enabled = !state.busy) { Text(stringResource(R.string.sync_disconnect)) }
            }
        }
        OutlinedButton(onClick = onRefresh, enabled = !state.busy) { Text(stringResource(R.string.action_refresh)) }
    }
}

@Composable
private fun SecretField(value: String, onChange: (String) -> Unit, label: Int) {
    OutlinedTextField(
        value,
        onChange,
        label = { Text(stringResource(label)) },
        visualTransformation = PasswordVisualTransformation(),
        modifier = Modifier.fillMaxWidth(),
    )
}
