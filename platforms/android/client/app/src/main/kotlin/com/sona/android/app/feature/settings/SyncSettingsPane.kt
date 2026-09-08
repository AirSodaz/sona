package com.sona.android.app.feature.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Key
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.QrCode
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Shield
import androidx.compose.material.icons.rounded.Sync
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material.icons.rounded.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sona.android.app.R
import com.sona.android.application.sync.DiscoveredVaultSummary
import com.sona.android.application.sync.SYNC_PROVIDER_PRESETS
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncLifecycleState
import com.sona.android.application.sync.SyncPairingPayload
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.WebDavSyncProvider
import com.sona.android.application.sync.WellKnownSyncProviderId
import com.sona.android.application.sync.detectProviderPresetId
import com.sona.android.application.sync.decodeSyncPairingToken
import com.sona.android.application.sync.encodeSyncPairingToken

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
    onDiscoverVaults: ((WebDavSyncProvider, ((List<DiscoveredVaultSummary>) -> Unit)?) -> Unit)? = null,
    onCreateWithVaultId: ((WebDavSyncProvider, SyncPreset, String, String?, Boolean) -> Unit)? = null,
    onApplyPairingToken: ((String) -> SyncPairingPayload?)? = null,
    onClearPairingNotice: (() -> Unit)? = null,
    onGetPairingToken: ((Boolean, String) -> String?)? = null,
) {
    var server by remember { mutableStateOf("") }
    var root by remember { mutableStateOf("Sona") }
    var username by remember { mutableStateOf("") }
    var providerPassword by remember { mutableStateOf("") }
    var masterPassword by remember { mutableStateOf("") }
    var vaultId by remember { mutableStateOf("") }
    var unlockMethod by remember { mutableStateOf("password") }
    var selectedPresetId by remember { mutableStateOf(WellKnownSyncProviderId.CUSTOM) }
    var showAdvanced by remember { mutableStateOf(false) }
    var createRecoveryKey by remember { mutableStateOf(true) }
    var selectedPresetScope by remember { mutableStateOf(SyncPreset.STANDARD) }

    // Dialog states
    var showImportPairingDialog by remember { mutableStateOf(false) }
    var pairingTokenInput by remember { mutableStateOf("") }
    var pairingTokenError by remember { mutableStateOf<String?>(null) }
    var showPairDeviceModal by remember { mutableStateOf(false) }
    var showDisconnectConfirm by remember { mutableStateOf(false) }

    // Multi-vault resolution dialog state
    var discoveredVaultsToResolve by remember { mutableStateOf<List<DiscoveredVaultSummary>?>(null) }
    var selectedVaultToJoin by remember { mutableStateOf("default") }
    var isCreatingNewVaultChoice by remember { mutableStateOf(false) }

    // Password change card states
    var currentPasswordInput by remember { mutableStateOf("") }
    var nextPasswordInput by remember { mutableStateOf("") }
    var confirmPasswordInput by remember { mutableStateOf("") }
    var passwordChangeMode by remember { mutableStateOf("password") }
    var passwordError by remember { mutableStateOf<String?>(null) }

    val provider = WebDavSyncProvider(server, root, username, providerPassword)
    val clipboard = LocalClipboardManager.current
    val keyExporter = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("text/plain"),
    ) { uri -> uri?.let { onExportRecoveryKey(it.toString()) } }

    val activePreset = remember(selectedPresetId) {
        SYNC_PROVIDER_PRESETS.firstOrNull { it.id == selectedPresetId }
    }

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        // Pairing Success Banner
        state.pairingSuccessNotice?.let { notice ->
            OutlinedCard(
                colors = CardDefaults.outlinedCardColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.3f),
                ),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.primary.copy(alpha = 0.5f)),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.padding(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(
                        Icons.Rounded.CheckCircle,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                    )
                    Text(
                        text = notice,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = { onClearPairingNotice?.invoke() }) {
                        Icon(Icons.Rounded.Close, contentDescription = null, modifier = Modifier.size(16.dp))
                    }
                }
            }
        }

        // Global error banner
        state.error?.let { err ->
            OutlinedCard(
                colors = CardDefaults.outlinedCardColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.3f),
                ),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.error),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    modifier = Modifier.padding(12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(Icons.Rounded.Warning, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                    Text(text = err, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }

        when (state.status.state) {
            SyncLifecycleState.DISABLED -> {
                // Top Pairing Quick Import Banner
                OutlinedCard(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.outlinedCardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.4f),
                    ),
                ) {
                    Row(
                        modifier = Modifier.padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.weight(1f),
                        ) {
                            Icon(
                                Icons.Rounded.Link,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.size(18.dp),
                            )
                            Text(
                                text = stringResource(R.string.sync_have_device_hint),
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                        OutlinedButton(
                            onClick = {
                                pairingTokenInput = ""
                                pairingTokenError = null
                                showImportPairingDialog = true
                            },
                            enabled = !state.busy,
                        ) {
                            Text(stringResource(R.string.sync_import_pairing_token))
                        }
                    }
                }

                // Provider Presets
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.sync_preset_label),
                        style = MaterialTheme.typography.labelLarge,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        SYNC_PROVIDER_PRESETS.forEach { preset ->
                            FilterChip(
                                selected = selectedPresetId == preset.id,
                                onClick = {
                                    selectedPresetId = preset.id
                                    server = preset.defaultServerUrl
                                    root = preset.defaultRemoteRoot
                                },
                                label = { Text(preset.defaultName) },
                            )
                        }
                    }
                    activePreset?.let { preset ->
                        if (preset.helpText.isNotBlank()) {
                            Text(
                                text = preset.helpText,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }

                // Connection Parameters
                OutlinedTextField(
                    value = server,
                    onValueChange = {
                        server = it
                        selectedPresetId = detectProviderPresetId(it)
                    },
                    label = { Text(stringResource(R.string.sync_server_url)) },
                    placeholder = { Text(activePreset?.defaultServerUrl ?: "https://...") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )

                OutlinedTextField(
                    value = username,
                    onValueChange = { username = it },
                    label = { Text(stringResource(R.string.sync_username)) },
                    placeholder = { Text(activePreset?.usernamePlaceholder ?: "user@example.com") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )

                SecretField(
                    value = providerPassword,
                    onChange = { providerPassword = it },
                    labelRes = R.string.sync_provider_password,
                )

                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(
                        onClick = { onTestProvider(provider) },
                        enabled = !state.busy && server.isNotBlank(),
                    ) {
                        Text(stringResource(R.string.sync_test))
                    }
                }

                HorizontalDivider()

                // Authentication Method (Master Password vs Emergency Recovery Key)
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        text = stringResource(R.string.sync_auth_mode),
                        style = MaterialTheme.typography.labelLarge,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(
                            selected = unlockMethod == "password",
                            onClick = { unlockMethod = "password" },
                            label = { Text(stringResource(R.string.sync_use_master_password)) },
                            leadingIcon = { Icon(Icons.Rounded.Key, contentDescription = null, modifier = Modifier.size(16.dp)) },
                        )
                        FilterChip(
                            selected = unlockMethod == "recovery",
                            onClick = { unlockMethod = "recovery" },
                            label = { Text(stringResource(R.string.sync_use_recovery_key)) },
                            leadingIcon = { Icon(Icons.Rounded.Shield, contentDescription = null, modifier = Modifier.size(16.dp)) },
                        )
                    }

                    SecretField(
                        value = masterPassword,
                        onChange = { masterPassword = it },
                        labelRes = if (unlockMethod == "password") R.string.sync_master_password else R.string.sync_recovery_key,
                        monospace = unlockMethod == "recovery",
                    )
                }

                // Advanced Settings Accordion
                OutlinedCard(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.outlinedCardColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                ) {
                    Column(modifier = Modifier.padding(14.dp)) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { showAdvanced = !showAdvanced },
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = stringResource(R.string.sync_advanced_title),
                                    style = MaterialTheme.typography.titleSmall,
                                    fontWeight = FontWeight.SemiBold,
                                )
                                Text(
                                    text = stringResource(R.string.sync_advanced_desc),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            IconButton(onClick = { showAdvanced = !showAdvanced }) {
                                Icon(
                                    if (showAdvanced) Icons.Rounded.KeyboardArrowUp else Icons.Rounded.KeyboardArrowDown,
                                    contentDescription = null,
                                )
                            }
                        }

                        if (showAdvanced) {
                            Spacer(modifier = Modifier.height(12.dp))
                            HorizontalDivider()
                            Spacer(modifier = Modifier.height(12.dp))

                            OutlinedTextField(
                                value = root,
                                onValueChange = { root = it },
                                label = { Text(stringResource(R.string.sync_remote_root)) },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )

                            Spacer(modifier = Modifier.height(8.dp))

                            OutlinedTextField(
                                value = vaultId,
                                onValueChange = { vaultId = it },
                                label = { Text(stringResource(R.string.sync_vault_id)) },
                                placeholder = { Text("Leave empty for auto-discovery or \"default\"") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )

                            Spacer(modifier = Modifier.height(8.dp))

                            Text(
                                text = stringResource(R.string.sync_preset),
                                style = MaterialTheme.typography.labelMedium,
                            )
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                SyncPreset.entries.forEach { p ->
                                    FilterChip(
                                        selected = selectedPresetScope == p,
                                        onClick = { selectedPresetScope = p },
                                        label = { Text(p.name.lowercase()) },
                                    )
                                }
                            }

                            Spacer(modifier = Modifier.height(8.dp))

                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = stringResource(R.string.sync_create_recovery_key_label),
                                        style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.Medium,
                                    )
                                    Text(
                                        text = stringResource(R.string.sync_create_recovery_key_hint),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                                Switch(
                                    checked = createRecoveryKey,
                                    onCheckedChange = { createRecoveryKey = it },
                                    enabled = !state.busy,
                                )
                            }
                        }
                    }
                }

                // Connect / Create Action Button
                val canSubmit = server.isNotBlank() && providerPassword.isNotBlank() && masterPassword.isNotBlank()
                Button(
                    onClick = {
                        val trimmedVault = vaultId.trim()
                        if (trimmedVault.isNotEmpty()) {
                            onJoin(provider, trimmedVault, masterPassword)
                        } else if (onDiscoverVaults != null) {
                            onDiscoverVaults(provider) { vaults ->
                                when {
                                    vaults.isEmpty() -> {
                                        if (onCreateWithVaultId != null) {
                                            onCreateWithVaultId(provider, selectedPresetScope, masterPassword, "default", createRecoveryKey)
                                        } else {
                                            onCreate(provider, selectedPresetScope, masterPassword)
                                        }
                                    }
                                    vaults.size == 1 -> {
                                        onJoin(provider, vaults[0].vaultId, masterPassword)
                                    }
                                    else -> {
                                        discoveredVaultsToResolve = vaults
                                        selectedVaultToJoin = vaults[0].vaultId
                                        isCreatingNewVaultChoice = false
                                    }
                                }
                            }
                        } else {
                            if (onCreateWithVaultId != null) {
                                onCreateWithVaultId(provider, selectedPresetScope, masterPassword, "default", createRecoveryKey)
                            } else {
                                onCreate(provider, selectedPresetScope, masterPassword)
                            }
                        }
                    },
                    enabled = !state.busy && canSubmit,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    if (state.busy) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp), color = MaterialTheme.colorScheme.onPrimary)
                        Spacer(modifier = Modifier.width(8.dp))
                    }
                    Text(stringResource(R.string.sync_connect))
                }
            }

            SyncLifecycleState.LOCKED -> {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                    ),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(Icons.Rounded.Lock, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            Text(
                                text = stringResource(R.string.sync_lock),
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }

                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            FilterChip(
                                selected = unlockMethod == "password",
                                onClick = { unlockMethod = "password" },
                                label = { Text(stringResource(R.string.sync_use_master_password)) },
                            )
                            FilterChip(
                                selected = unlockMethod == "recovery",
                                onClick = { unlockMethod = "recovery" },
                                label = { Text(stringResource(R.string.sync_use_recovery_key)) },
                            )
                        }

                        SecretField(
                            value = providerPassword,
                            onChange = { providerPassword = it },
                            labelRes = R.string.sync_provider_password,
                        )

                        SecretField(
                            value = masterPassword,
                            onChange = { masterPassword = it },
                            labelRes = if (unlockMethod == "recovery") R.string.sync_recovery_key else R.string.sync_master_password,
                            monospace = unlockMethod == "recovery",
                        )

                        Button(
                            onClick = {
                                if (unlockMethod == "recovery") {
                                    onUnlockWithRecovery(providerPassword, masterPassword)
                                } else {
                                    onUnlock(providerPassword, masterPassword)
                                }
                            },
                            enabled = !state.busy && providerPassword.isNotBlank() && masterPassword.isNotBlank(),
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            if (state.busy) {
                                CircularProgressIndicator(modifier = Modifier.size(16.dp), color = MaterialTheme.colorScheme.onPrimary)
                                Spacer(modifier = Modifier.width(8.dp))
                            }
                            Text(stringResource(R.string.sync_unlock))
                        }
                    }
                }
            }

            else -> {
                // Connected Hero Card
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                    ),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                val dotColor = when (state.status.state) {
                                    SyncLifecycleState.IDLE -> Color(0xFF22C55E)
                                    SyncLifecycleState.SYNCING -> MaterialTheme.colorScheme.primary
                                    SyncLifecycleState.PAUSED -> Color(0xFFF59E0B)
                                    SyncLifecycleState.ERROR -> MaterialTheme.colorScheme.error
                                    else -> MaterialTheme.colorScheme.outline
                                }
                                Surface(
                                    modifier = Modifier.size(10.dp),
                                    shape = CircleShape,
                                    color = dotColor,
                                ) {}
                                Text(
                                    text = state.status.state.name,
                                    style = MaterialTheme.typography.titleMedium,
                                    fontWeight = FontWeight.SemiBold,
                                )
                            }

                            OutlinedButton(
                                onClick = { showPairDeviceModal = true },
                                enabled = !state.busy,
                            ) {
                                Icon(Icons.Rounded.QrCode, contentDescription = null, modifier = Modifier.size(16.dp))
                                Spacer(modifier = Modifier.width(6.dp))
                                Text(stringResource(R.string.sync_pair_btn))
                            }
                        }

                        HorizontalDivider()

                        state.pairingInfo?.serverUrl?.let { url ->
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Text(stringResource(R.string.sync_server_url), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Text(url, style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.Medium)
                            }
                        }

                        state.pairingInfo?.username?.let { user ->
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Text(stringResource(R.string.sync_username), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Text(user, style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.Medium)
                            }
                        }

                        state.status.vaultId?.let { id ->
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Text(stringResource(R.string.sync_vault_id), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Text(id, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                            }
                        }

                        Text(
                            text = stringResource(R.string.sync_counts, state.status.pendingOperationCount, state.status.conflictCount),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // Actions Bar (Sync now, Pause/Resume, Lock)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Button(
                        onClick = onRunNow,
                        enabled = !state.busy && state.status.state != SyncLifecycleState.PAUSED,
                        modifier = Modifier.weight(1f),
                    ) {
                        Icon(Icons.Rounded.Sync, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(stringResource(R.string.sync_now))
                    }

                    OutlinedButton(
                        onClick = { onSetPaused(state.status.state != SyncLifecycleState.PAUSED) },
                        enabled = !state.busy,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(stringResource(if (state.status.state == SyncLifecycleState.PAUSED) R.string.sync_resume else R.string.sync_pause))
                    }

                    OutlinedButton(
                        onClick = onLock,
                        enabled = !state.busy,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(stringResource(R.string.sync_lock))
                    }
                }

                // Scope / Preset Selection
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(stringResource(R.string.sync_preset), style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SyncPreset.entries.forEach { p ->
                            FilterChip(
                                selected = state.status.preset == p,
                                onClick = { onChangePreset(p) },
                                enabled = !state.busy,
                                label = { Text(p.name.lowercase()) },
                            )
                        }
                    }
                }

                // Security & Recovery Card
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(Icons.Rounded.Shield, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            Text(
                                text = stringResource(R.string.sync_security_title),
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }
                        Text(
                            text = stringResource(R.string.sync_security_hint),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )

                        HorizontalDivider()

                        // Recovery Key Management
                        Text(
                            text = stringResource(R.string.sync_recovery_key_manage_title),
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Medium,
                        )
                        Text(
                            text = stringResource(R.string.sync_recovery_key_manage_hint),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )

                        OutlinedButton(
                            onClick = onGenerateRecoveryKey,
                            enabled = !state.busy,
                        ) {
                            Text(stringResource(R.string.sync_generate_recovery_key))
                        }

                        state.recoveryKey?.let { key ->
                            OutlinedCard(
                                modifier = Modifier.fillMaxWidth(),
                                colors = CardDefaults.outlinedCardColors(
                                    containerColor = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.2f),
                                ),
                            ) {
                                Column(
                                    modifier = Modifier.padding(12.dp),
                                    verticalArrangement = Arrangement.spacedBy(8.dp),
                                ) {
                                    Text(
                                        text = stringResource(R.string.sync_active_recovery_key),
                                        style = MaterialTheme.typography.labelMedium,
                                        fontWeight = FontWeight.SemiBold,
                                    )
                                    Text(
                                        text = key,
                                        style = MaterialTheme.typography.bodySmall,
                                        fontFamily = FontFamily.Monospace,
                                    )
                                    Text(
                                        text = stringResource(R.string.sync_recovery_key_save_warning),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                        OutlinedButton(
                                            onClick = {
                                                clipboard.setText(AnnotatedString(key))
                                                onConsumeRecoveryKey()
                                            },
                                        ) {
                                            Icon(Icons.Rounded.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                                            Spacer(modifier = Modifier.width(4.dp))
                                            Text(stringResource(R.string.action_copy))
                                        }
                                        OutlinedButton(
                                            onClick = { keyExporter.launch("sona-recovery-key.txt") },
                                        ) {
                                            Icon(Icons.Rounded.Download, contentDescription = null, modifier = Modifier.size(16.dp))
                                            Spacer(modifier = Modifier.width(4.dp))
                                            Text(stringResource(R.string.action_export))
                                        }
                                    }
                                }
                            }
                        }

                        HorizontalDivider()

                        // Change Master Password Form
                        Text(
                            text = stringResource(R.string.sync_change_master_password),
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Medium,
                        )

                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            FilterChip(
                                selected = passwordChangeMode == "password",
                                onClick = { passwordChangeMode = "password" },
                                label = { Text(stringResource(R.string.sync_change_password_mode_current)) },
                            )
                            FilterChip(
                                selected = passwordChangeMode == "recovery",
                                onClick = { passwordChangeMode = "recovery" },
                                label = { Text(stringResource(R.string.sync_change_password_mode_recovery)) },
                            )
                        }

                        SecretField(
                            value = currentPasswordInput,
                            onChange = {
                                currentPasswordInput = it
                                passwordError = null
                            },
                            labelRes = if (passwordChangeMode == "password") R.string.sync_current_master_password else R.string.sync_recovery_key,
                            monospace = passwordChangeMode == "recovery",
                        )

                        SecretField(
                            value = nextPasswordInput,
                            onChange = {
                                nextPasswordInput = it
                                passwordError = null
                            },
                            labelRes = R.string.sync_new_master_password,
                        )

                        SecretField(
                            value = confirmPasswordInput,
                            onChange = {
                                confirmPasswordInput = it
                                passwordError = null
                            },
                            labelRes = R.string.sync_confirm_new_password,
                        )

                        passwordError?.let { err ->
                            Text(text = err, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                        }

                        val canUpdatePassword = currentPasswordInput.isNotBlank() &&
                            nextPasswordInput.isNotBlank() &&
                            confirmPasswordInput.isNotBlank() &&
                            !state.busy

                        val mismatchMessage = stringResource(R.string.sync_password_mismatch)
                        OutlinedButton(
                            onClick = {
                                if (nextPasswordInput != confirmPasswordInput) {
                                    passwordError = mismatchMessage
                                    return@OutlinedButton
                                }
                                onChangePassword(currentPasswordInput, nextPasswordInput)
                                currentPasswordInput = ""
                                nextPasswordInput = ""
                                confirmPasswordInput = ""
                                passwordError = null
                            },
                            enabled = canUpdatePassword,
                        ) {
                            Text(stringResource(R.string.sync_change_master_password))
                        }
                    }
                }

                // Conflicts Center
                if (state.conflicts.isNotEmpty()) {
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                    ) {
                        Column(
                            modifier = Modifier.padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Text(
                                text = "Conflict Center (${state.conflicts.size})",
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.SemiBold,
                            )

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
                        }
                    }
                }

                // Danger Zone (Disconnect)
                OutlinedCard(
                    modifier = Modifier.fillMaxWidth(),
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.error.copy(alpha = 0.6f)),
                    colors = CardDefaults.outlinedCardColors(
                        containerColor = MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.1f),
                    ),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.sync_danger_zone),
                            style = MaterialTheme.typography.titleSmall,
                            color = MaterialTheme.colorScheme.error,
                            fontWeight = FontWeight.SemiBold,
                        )
                        Text(
                            text = stringResource(R.string.sync_disconnect_warning),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { showDisconnectConfirm = true },
                            enabled = !state.busy,
                            colors = ButtonDefaults.outlinedButtonColors(
                                contentColor = MaterialTheme.colorScheme.error,
                            ),
                            border = BorderStroke(1.dp, MaterialTheme.colorScheme.error),
                        ) {
                            Text(stringResource(R.string.sync_disconnect))
                        }
                    }
                }
            }
        }

        OutlinedButton(
            onClick = onRefresh,
            enabled = !state.busy,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Icon(Icons.Rounded.Refresh, contentDescription = null, modifier = Modifier.size(16.dp))
            Spacer(modifier = Modifier.width(6.dp))
            Text(stringResource(R.string.action_refresh))
        }
    }

    // Modal 1: Import Pairing Token Dialog
    if (showImportPairingDialog) {
        val invalidTokenMessage = "Invalid pairing token format."
        AlertDialog(
            onDismissRequest = { showImportPairingDialog = false },
            title = { Text(stringResource(R.string.sync_pairing_modal_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = stringResource(R.string.sync_pairing_modal_desc),
                        style = MaterialTheme.typography.bodySmall,
                    )
                    OutlinedTextField(
                        value = pairingTokenInput,
                        onValueChange = {
                            pairingTokenInput = it
                            pairingTokenError = null
                        },
                        label = { Text(stringResource(R.string.sync_pairing_token_label)) },
                        placeholder = { Text("sonasync://v1?data=...") },
                        modifier = Modifier.fillMaxWidth(),
                        maxLines = 3,
                    )
                    OutlinedButton(
                        onClick = {
                            val clip = clipboard.getText()?.text?.trim().orEmpty()
                            if (clip.isNotEmpty()) {
                                pairingTokenInput = clip
                                pairingTokenError = null
                            }
                        },
                    ) {
                        Icon(Icons.Rounded.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text("Paste from clipboard")
                    }
                    pairingTokenError?.let { err ->
                        Text(text = err, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Icon(Icons.Rounded.Info, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp))
                        Text(
                            text = stringResource(R.string.sync_pairing_security_note),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            },
            confirmButton = {
                Button(
                    onClick = {
                        val payload = onApplyPairingToken?.invoke(pairingTokenInput)
                            ?: decodeSyncPairingToken(pairingTokenInput)
                        if (payload != null) {
                            server = payload.serverUrl
                            root = payload.remoteRoot
                            username = payload.username
                            vaultId = payload.vaultId
                            payload.providerPassword?.let { providerPassword = it }
                            selectedPresetId = detectProviderPresetId(payload.serverUrl)
                            showImportPairingDialog = false
                        } else {
                            pairingTokenError = invalidTokenMessage
                        }
                    },
                    enabled = pairingTokenInput.isNotBlank(),
                ) {
                    Text("Import")
                }
            },
            dismissButton = {
                TextButton(onClick = { showImportPairingDialog = false }) {
                    Text("Cancel")
                }
            },
        )
    }

    // Modal 2: Pair New Device Modal
    if (showPairDeviceModal) {
        val pairingToken = remember(state.pairingInfo, state.status.vaultId) {
            onGetPairingToken?.invoke(false, "") ?: state.pairingInfo?.let { info ->
                info.serverUrl?.let { s ->
                    encodeSyncPairingToken(
                        serverUrl = s,
                        remoteRoot = info.remoteRoot ?: "Sona",
                        username = info.username ?: "",
                        vaultId = info.vaultId,
                    )
                }
            }.orEmpty()
        }
        var copiedNotice by remember { mutableStateOf(false) }

        AlertDialog(
            onDismissRequest = { showPairDeviceModal = false },
            title = { Text(stringResource(R.string.sync_pair_device_modal_title)) },
            text = {
                Column(
                    modifier = Modifier.verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = "Use this pairing code to quickly connect your second device without re-entering server parameters.",
                        style = MaterialTheme.typography.bodySmall,
                    )

                    OutlinedCard(modifier = Modifier.fillMaxWidth()) {
                        Column(modifier = Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            state.pairingInfo?.serverUrl?.let {
                                Text("Server: $it", style = MaterialTheme.typography.bodySmall)
                            }
                            state.pairingInfo?.username?.let {
                                Text("Username: $it", style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.Medium)
                            }
                            state.status.vaultId?.let {
                                Text("Vault ID: $it", style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                            }
                        }
                    }

                    OutlinedTextField(
                        value = pairingToken,
                        onValueChange = {},
                        readOnly = true,
                        label = { Text(stringResource(R.string.sync_pairing_token_label)) },
                        modifier = Modifier.fillMaxWidth(),
                        maxLines = 3,
                    )

                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = {
                                clipboard.setText(AnnotatedString(pairingToken))
                                copiedNotice = true
                            },
                        ) {
                            Icon(if (copiedNotice) Icons.Rounded.Check else Icons.Rounded.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                            Spacer(modifier = Modifier.width(4.dp))
                            Text(if (copiedNotice) "Copied" else stringResource(R.string.sync_copy_pairing_token))
                        }
                    }

                    HorizontalDivider()

                    Text(stringResource(R.string.sync_pair_step_1), style = MaterialTheme.typography.bodySmall)
                    Text(stringResource(R.string.sync_pair_step_2), style = MaterialTheme.typography.bodySmall)
                    Text(stringResource(R.string.sync_pair_step_3), style = MaterialTheme.typography.bodySmall)
                }
            },
            confirmButton = {
                TextButton(onClick = { showPairDeviceModal = false }) {
                    Text("Close")
                }
            },
        )
    }

    // Modal 3: Multi-Vault Selection Modal
    discoveredVaultsToResolve?.let { vaults ->
        AlertDialog(
            onDismissRequest = { discoveredVaultsToResolve = null },
            title = { Text(stringResource(R.string.sync_multi_vault_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = stringResource(R.string.sync_multi_vault_desc),
                        style = MaterialTheme.typography.bodySmall,
                    )

                    vaults.forEach { v ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    isCreatingNewVaultChoice = false
                                    selectedVaultToJoin = v.vaultId
                                },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            RadioButton(
                                selected = !isCreatingNewVaultChoice && selectedVaultToJoin == v.vaultId,
                                onClick = {
                                    isCreatingNewVaultChoice = false
                                    selectedVaultToJoin = v.vaultId
                                },
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Column {
                                Text(v.vaultId, fontWeight = FontWeight.Medium, fontFamily = FontFamily.Monospace)
                                Text("Preset: ${v.preset.name.lowercase()}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }

                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { isCreatingNewVaultChoice = true },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(
                            selected = isCreatingNewVaultChoice,
                            onClick = { isCreatingNewVaultChoice = true },
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(stringResource(R.string.sync_create_new_vault_option), fontWeight = FontWeight.Medium)
                    }
                }
            },
            confirmButton = {
                Button(
                    onClick = {
                        val choiceCreate = isCreatingNewVaultChoice
                        val vaultToJoin = selectedVaultToJoin
                        discoveredVaultsToResolve = null
                        if (choiceCreate) {
                            if (onCreateWithVaultId != null) {
                                onCreateWithVaultId(provider, selectedPresetScope, masterPassword, null, createRecoveryKey)
                            } else {
                                onCreate(provider, selectedPresetScope, masterPassword)
                            }
                        } else {
                            onJoin(provider, vaultToJoin, masterPassword)
                        }
                    },
                ) {
                    Text(if (isCreatingNewVaultChoice) stringResource(R.string.sync_create) else stringResource(R.string.sync_join_selected_vault))
                }
            },
            dismissButton = {
                TextButton(onClick = { discoveredVaultsToResolve = null }) {
                    Text("Cancel")
                }
            },
        )
    }

    // Modal 4: Disconnect Confirmation Dialog
    if (showDisconnectConfirm) {
        AlertDialog(
            onDismissRequest = { showDisconnectConfirm = false },
            title = { Text(stringResource(R.string.sync_disconnect)) },
            text = { Text(stringResource(R.string.sync_disconnect_warning)) },
            confirmButton = {
                Button(
                    onClick = {
                        showDisconnectConfirm = false
                        onDisconnect()
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error),
                ) {
                    Text(stringResource(R.string.sync_disconnect))
                }
            },
            dismissButton = {
                TextButton(onClick = { showDisconnectConfirm = false }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
private fun SecretField(
    value: String,
    onChange: (String) -> Unit,
    labelRes: Int,
    monospace: Boolean = false,
) {
    var passwordVisible by remember { mutableStateOf(false) }

    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(stringResource(labelRes)) },
        singleLine = true,
        textStyle = if (monospace) {
            MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace)
        } else {
            MaterialTheme.typography.bodyMedium
        },
        visualTransformation = if (passwordVisible) VisualTransformation.None else PasswordVisualTransformation(),
        trailingIcon = {
            val image = if (passwordVisible) Icons.Rounded.VisibilityOff else Icons.Rounded.Visibility
            IconButton(onClick = { passwordVisible = !passwordVisible }) {
                Icon(image, contentDescription = if (passwordVisible) "Hide password" else "Show password")
            }
        },
        modifier = Modifier.fillMaxWidth(),
    )
}
