package com.sona.android.app.feature.settings

import android.content.ClipData
import android.content.ClipboardManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.CleaningServices
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.ui.component.InsetGroupedCard
import com.sona.android.app.ui.component.SonaOptionItem
import com.sona.android.app.ui.component.SonaOptionSettingRow
import com.sona.android.app.ui.component.SonaSingleChoiceDialog
import com.sona.android.app.ui.component.SonaSwitch
import com.sona.android.app.ui.component.TwoLineItemDivider
import com.sona.android.app.ui.component.springOverscroll
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.DownloadMirror
import java.util.Locale

@Composable
internal fun GeneralDiagnosticsPane(
    state: GeneralDiagnosticsUiState,
    onShown: () -> Unit,
    onAutoCheckUpdatesChanged: (Boolean) -> Unit,
    onWifiOnlyUpdatesChanged: (Boolean) -> Unit,
    onDownloadMirrorChanged: (DownloadMirror) -> Unit,
    onLogLevelChanged: (AppLogLevel) -> Unit,
    onClearIgnoredUpdate: () -> Unit,
    onRefreshDiagnostics: () -> Unit,
    onClearTempCaches: () -> Unit,
    onExportLogs: (String) -> Unit,
    onClearLogs: () -> Unit,
    onOpenRecognitionSettings: () -> Unit,
    onBuildDiagnosticReport: () -> String,
    onDismissNotice: () -> Unit,
    modifier: Modifier = Modifier,
    extraBottomPadding: Dp = 0.dp,
) {
    val context = LocalContext.current
    val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    var mirrorDialogVisible by rememberSaveable { mutableStateOf(false) }
    var logLevelDialogVisible by rememberSaveable { mutableStateOf(false) }
    var copiedBanner by rememberSaveable { mutableStateOf(false) }

    val logExporter = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("text/plain"),
    ) { uri -> uri?.let { onExportLogs(it.toString()) } }

    LaunchedEffect(Unit) { onShown() }

    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .widthIn(max = 720.dp)
                .fillMaxWidth()
                .springOverscroll()
                .verticalScroll(rememberScrollState())
                .padding(
                    start = 24.dp,
                    end = 24.dp,
                    top = 20.dp,
                    bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
                ),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            state.notice?.let { notice ->
                val isError = notice == GeneralDiagnosticsNotice.OPERATION_FAILED
                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = if (isError) {
                        MaterialTheme.colorScheme.errorContainer
                    } else {
                        MaterialTheme.colorScheme.secondaryContainer
                    },
                    contentColor = if (isError) {
                        MaterialTheme.colorScheme.onErrorContainer
                    } else {
                        MaterialTheme.colorScheme.onSecondaryContainer
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = when (notice) {
                                GeneralDiagnosticsNotice.CACHE_CLEARED -> stringResource(
                                    R.string.general_cache_cleared_notice,
                                    formatByteSize(state.lastFreedBytes),
                                )
                                GeneralDiagnosticsNotice.LOGS_EXPORTED ->
                                    stringResource(R.string.general_logs_exported_notice)
                                GeneralDiagnosticsNotice.LOGS_CLEARED ->
                                    stringResource(R.string.general_logs_cleared_notice)
                                GeneralDiagnosticsNotice.OPERATION_FAILED ->
                                    stringResource(R.string.general_operation_failed)
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.weight(1f),
                        )
                        TextButton(onClick = onDismissNotice) {
                            Text(stringResource(R.string.action_dismiss))
                        }
                    }
                }
            }

            // 1. Updates & Downloads Card
            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = stringResource(R.string.general_updates_downloads_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 18.dp),
                    )
                    SwitchSettingRow(
                        title = stringResource(R.string.general_auto_check_updates),
                        description = stringResource(R.string.general_auto_check_updates_desc),
                        checked = state.settings.autoCheckUpdates,
                        onCheckedChange = onAutoCheckUpdatesChanged,
                    )
                    TwoLineItemDivider(startIndent = 18.dp)
                    SwitchSettingRow(
                        title = stringResource(R.string.general_wifi_only_updates),
                        description = stringResource(R.string.general_wifi_only_updates_desc),
                        checked = state.settings.wifiOnlyUpdates,
                        onCheckedChange = onWifiOnlyUpdatesChanged,
                    )
                    TwoLineItemDivider(startIndent = 18.dp)
                    SonaOptionSettingRow(
                        headline = stringResource(R.string.general_download_mirror),
                        selectedValueText = stringResource(state.settings.downloadMirror.labelRes()),
                        onClick = { mirrorDialogVisible = true },
                    )
                    state.settings.ignoredUpdateVersionCode?.let { ignoredCode ->
                        TwoLineItemDivider(startIndent = 18.dp)
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 18.dp, vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = stringResource(R.string.general_ignored_version_title),
                                    style = MaterialTheme.typography.bodyLarge,
                                )
                                Text(
                                    text = stringResource(R.string.general_ignored_version_desc, ignoredCode),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            TextButton(onClick = onClearIgnoredUpdate) {
                                Text(stringResource(R.string.general_ignored_version_reset))
                            }
                        }
                    }
                }
            }

            // 2. Logging Card
            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = stringResource(R.string.general_logging_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 18.dp),
                    )
                    SonaOptionSettingRow(
                        headline = stringResource(R.string.general_log_level),
                        selectedValueText = stringResource(state.settings.logLevel.labelRes()),
                        onClick = { logLevelDialogVisible = true },
                    )
                    TwoLineItemDivider(startIndent = 18.dp)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 18.dp, vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = stringResource(R.string.general_log_file_title),
                                style = MaterialTheme.typography.bodyLarge,
                            )
                            Text(
                                text = stringResource(
                                    R.string.general_log_file_size,
                                    formatByteSize(state.logSizeBytes),
                                ),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        OutlinedButton(
                            onClick = onClearLogs,
                            enabled = state.logSizeBytes > 0L && !state.exportingLogs,
                        ) {
                            Icon(Icons.Rounded.DeleteOutline, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(stringResource(R.string.general_logs_clear))
                        }
                        FilledTonalButton(
                            onClick = { logExporter.launch("sona-android-logs.txt") },
                            enabled = !state.exportingLogs,
                        ) {
                            Icon(Icons.Rounded.Download, contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(stringResource(R.string.general_logs_export))
                        }
                    }
                }
            }

            // 3. Diagnostics Card
            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(18.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = stringResource(R.string.general_diagnostics_heading),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(
                            onClick = {
                                val report = onBuildDiagnosticReport()
                                val clipboard = context.getSystemService(ClipboardManager::class.java)
                                clipboard?.setPrimaryClip(ClipData.newPlainText("Sona Diagnostics", report))
                                copiedBanner = true
                            },
                        ) {
                            Icon(
                                Icons.Rounded.ContentCopy,
                                contentDescription = stringResource(R.string.general_diagnostics_copy),
                            )
                        }
                        IconButton(
                            onClick = onRefreshDiagnostics,
                            enabled = !state.loadingDiagnostics,
                        ) {
                            if (state.loadingDiagnostics) {
                                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                            } else {
                                Icon(
                                    Icons.Rounded.Refresh,
                                    contentDescription = stringResource(R.string.general_diagnostics_refresh),
                                )
                            }
                        }
                    }

                    if (copiedBanner) {
                        Text(
                            text = stringResource(R.string.sync_copied),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary,
                        )
                    }

                    val diag = state.diagnosticsSummary
                    if (diag == null) {
                        Text(
                            text = stringResource(R.string.general_diagnostics_loading),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        val ready = diag.overallStatus == "ready"
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = if (ready) Icons.Rounded.CheckCircle else Icons.Rounded.WarningAmber,
                                contentDescription = null,
                                tint = if (ready) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
                                modifier = Modifier.size(20.dp),
                            )
                            Text(
                                text = stringResource(
                                    if (ready) {
                                        R.string.general_diagnostics_ready
                                    } else {
                                        R.string.general_diagnostics_attention
                                    },
                                ),
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = FontWeight.Medium,
                            )
                        }

                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_diag_device),
                            value = "${diag.deviceAbi} · ${diag.deviceTier.uppercase(Locale.US)} (${diag.cpuCores} cores, ${formatByteSize(diag.totalMemoryBytes)})",
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_diag_mic_permission),
                            value = stringResource(
                                if (diag.microphonePermissionGranted) {
                                    R.string.general_diag_granted
                                } else {
                                    R.string.general_diag_missing
                                },
                            ),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_diag_notif_permission),
                            value = stringResource(
                                if (diag.notificationPermissionGranted) {
                                    R.string.general_diag_granted
                                } else {
                                    R.string.general_diag_missing
                                },
                            ),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_diag_live_model),
                            value = formatModelCheckValue(diag.liveModelCheck.label, diag.liveModelCheck.detail),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_diag_batch_model),
                            value = formatModelCheckValue(diag.batchModelCheck.label, diag.batchModelCheck.detail),
                        )

                        if (!diag.liveModelCheck.ready || !diag.batchModelCheck.ready) {
                            FilledTonalButton(onClick = onOpenRecognitionSettings) {
                                Text(stringResource(R.string.general_diag_open_recognition))
                            }
                        }
                    }
                }
            }

            // 4. Storage & Cache Card
            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(18.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = stringResource(R.string.general_storage_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    val storage = state.storageSummary
                    if (storage == null) {
                        Text(
                            text = stringResource(R.string.general_storage_loading),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_storage_total),
                            value = formatByteSize(storage.totalAppBytes),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_storage_recordings),
                            value = formatByteSize(storage.audioBytes),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_storage_models),
                            value = formatByteSize(storage.localModelsBytes),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_storage_database),
                            value = formatByteSize(storage.databaseBytes),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_storage_cache),
                            value = formatByteSize(storage.updateCacheBytes + storage.tempCacheBytes),
                        )
                        DiagnosticKeyValueRow(
                            label = stringResource(R.string.general_storage_free),
                            value = formatByteSize(storage.availableDeviceBytes),
                        )
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.End,
                        ) {
                            FilledTonalButton(
                                onClick = onClearTempCaches,
                                enabled = !state.clearingCache &&
                                    (storage.updateCacheBytes + storage.tempCacheBytes) > 0L,
                                colors = ButtonDefaults.filledTonalButtonColors(),
                            ) {
                                Icon(
                                    Icons.Rounded.CleaningServices,
                                    contentDescription = null,
                                    modifier = Modifier.size(18.dp),
                                )
                                Spacer(Modifier.width(6.dp))
                                Text(stringResource(R.string.general_storage_clear_cache))
                            }
                        }
                    }
                }
            }
        }
    }

    if (mirrorDialogVisible) {
        val options = DownloadMirror.entries.map { mirror ->
            SonaOptionItem(
                value = mirror,
                label = stringResource(mirror.labelRes()),
                supportingText = stringResource(mirror.descriptionRes()),
            )
        }
        SonaSingleChoiceDialog(
            title = stringResource(R.string.general_download_mirror),
            options = options,
            selectedValue = state.settings.downloadMirror,
            onSelect = onDownloadMirrorChanged,
            onDismiss = { mirrorDialogVisible = false },
        )
    }

    if (logLevelDialogVisible) {
        val options = AppLogLevel.entries.map { level ->
            SonaOptionItem(
                value = level,
                label = stringResource(level.labelRes()),
            )
        }
        SonaSingleChoiceDialog(
            title = stringResource(R.string.general_log_level),
            options = options,
            selectedValue = state.settings.logLevel,
            onSelect = onLogLevelChanged,
            onDismiss = { logLevelDialogVisible = false },
        )
    }
}

@Composable
private fun SwitchSettingRow(
    title: String,
    description: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(
                value = checked,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .padding(horizontal = 18.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.width(16.dp))
        SonaSwitch(
            checked = checked,
            onCheckedChange = null,
        )
    }
}

@Composable
private fun DiagnosticKeyValueRow(
    label: String,
    value: String,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(end = 12.dp),
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun formatModelCheckValue(label: String, detail: String): String {
    val statusText = when (detail) {
        "installed", "credential_configured" -> stringResource(R.string.general_diag_status_ready)
        "credential_missing" -> stringResource(R.string.general_diag_status_missing_key)
        "model_missing" -> stringResource(R.string.general_diag_status_missing_model)
        else -> stringResource(R.string.general_diag_status_unselected)
    }
    return "$label ($statusText)"
}

internal fun formatByteSize(bytes: Long): String {
    if (bytes < 1024L) return "$bytes B"
    val kb = bytes / 1024.0
    if (kb < 1024.0) return String.format(Locale.US, "%.1f KB", kb)
    val mb = kb / 1024.0
    if (mb < 1024.0) return String.format(Locale.US, "%.1f MB", mb)
    val gb = mb / 1024.0
    return String.format(Locale.US, "%.2f GB", gb)
}

@StringRes
internal fun DownloadMirror.labelRes(): Int = when (this) {
    DownloadMirror.AUTO -> R.string.mirror_auto
    DownloadMirror.DIRECT -> R.string.mirror_direct
    DownloadMirror.GH_PROXY -> R.string.mirror_ghproxy
    DownloadMirror.GH_NET -> R.string.mirror_ghnet
    DownloadMirror.HF_MIRROR -> R.string.mirror_hfmirror
}

@StringRes
private fun DownloadMirror.descriptionRes(): Int = when (this) {
    DownloadMirror.AUTO -> R.string.mirror_auto_desc
    DownloadMirror.DIRECT -> R.string.mirror_direct_desc
    DownloadMirror.GH_PROXY -> R.string.mirror_ghproxy_desc
    DownloadMirror.GH_NET -> R.string.mirror_ghnet_desc
    DownloadMirror.HF_MIRROR -> R.string.mirror_hfmirror_desc
}

@StringRes
internal fun AppLogLevel.labelRes(): Int = when (this) {
    AppLogLevel.TRACE -> R.string.log_level_trace
    AppLogLevel.DEBUG -> R.string.log_level_debug
    AppLogLevel.INFO -> R.string.log_level_info
    AppLogLevel.WARN -> R.string.log_level_warn
    AppLogLevel.ERROR -> R.string.log_level_error
}
