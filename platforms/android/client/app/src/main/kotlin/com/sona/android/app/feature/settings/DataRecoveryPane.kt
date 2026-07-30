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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoveryUnavailableReason

@Composable
internal fun DataRecoveryPane(
    state: DataRecoveryUiState,
    onExportBackup: (String) -> Unit,
    onInspectBackup: (String) -> Unit,
    onConfirmImport: () -> Unit,
    onCancelImport: () -> Unit,
    onRefresh: () -> Unit,
    onResume: (String) -> Unit,
    onResumeAll: () -> Unit,
    onDiscard: (String) -> Unit,
    onClearResolved: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val backupExporter = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/x-bzip2"),
    ) { uri -> uri?.let { onExportBackup(it.toString()) } }
    val backupImporter = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri -> uri?.let { onInspectBackup(it.toString()) } }

    Column(
        modifier = modifier.verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(stringResource(R.string.backup_heading), style = MaterialTheme.typography.titleMedium)
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        if (state.blockers.isNotEmpty()) Text(stringResource(R.string.backup_blocked), color = MaterialTheme.colorScheme.tertiary)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = { backupExporter.launch("sona-backup.tar.bz2") }, enabled = !state.busy && state.blockers.isEmpty()) { Text(stringResource(R.string.backup_export)) }
            OutlinedButton(onClick = { backupImporter.launch(arrayOf("application/x-bzip2", "application/octet-stream")) }, enabled = !state.busy && state.blockers.isEmpty()) { Text(stringResource(R.string.backup_import)) }
        }
        Text(stringResource(R.string.backup_excludes), style = MaterialTheme.typography.bodySmall)
        Text(stringResource(R.string.recovery_heading), style = MaterialTheme.typography.titleMedium)
        if (state.recovery.items.any { it.resolution == RecoveryResolution.PENDING && it.canResume }) {
            Button(onClick = onResumeAll, enabled = !state.busy) {
                Text(stringResource(R.string.recovery_resume_all))
            }
        }
        state.recovery.items.forEach { item ->
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(item.filename, style = MaterialTheme.typography.titleSmall)
                Text("${item.stage.name} - ${(item.progress * 100).toInt()}%", style = MaterialTheme.typography.bodySmall)
                item.unavailableReason?.let { reason ->
                    Text(
                        stringResource(when (reason) {
                            RecoveryUnavailableReason.SOURCE_MISSING -> R.string.recovery_unavailable_source
                            RecoveryUnavailableReason.MODEL_MISSING -> R.string.recovery_unavailable_model
                            RecoveryUnavailableReason.CREDENTIAL_MISSING -> R.string.recovery_unavailable_credential
                            RecoveryUnavailableReason.AUTOMATION_UNSUPPORTED -> R.string.recovery_unavailable_automation
                            RecoveryUnavailableReason.INVALID_PAYLOAD -> R.string.recovery_unavailable_payload
                            RecoveryUnavailableReason.HISTORY_MISSING -> R.string.recovery_unavailable_history
                            RecoveryUnavailableReason.TRANSCRIPT_CHANGED -> R.string.recovery_unavailable_transcript_changed
                        }),
                        color = MaterialTheme.colorScheme.tertiary,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (item.resolution == RecoveryResolution.PENDING) {
                        Button(onClick = { onResume(item.id) }, enabled = item.canResume && !state.busy) { Text(stringResource(R.string.recovery_resume)) }
                        OutlinedButton(onClick = { onDiscard(item.id) }, enabled = !state.busy) { Text(stringResource(R.string.recovery_discard)) }
                    }
                }
            }
        }
        OutlinedButton(onClick = onRefresh, enabled = !state.busy) { Text(stringResource(R.string.action_refresh)) }
        OutlinedButton(onClick = onClearResolved, enabled = !state.busy) { Text(stringResource(R.string.recovery_clear_resolved)) }
    }

    state.preparedBackup?.let { prepared ->
        AlertDialog(
            onDismissRequest = onCancelImport,
            title = { Text(stringResource(R.string.backup_confirm_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(
                        R.string.backup_manifest_details,
                        prepared.manifest.schemaVersion,
                        prepared.manifest.createdAt,
                        prepared.manifest.appVersion,
                    ))
                    Text(stringResource(
                        R.string.backup_confirm_body,
                        prepared.manifest.counts.historyItems,
                        prepared.manifest.counts.transcriptFiles,
                        prepared.manifest.counts.tags,
                    ))
                    Text(stringResource(R.string.backup_replace_scopes))
                    Text(stringResource(R.string.backup_excludes))
                }
            },
            confirmButton = { Button(onClick = onConfirmImport) { Text(stringResource(R.string.backup_restore)) } },
            dismissButton = { TextButton(onClick = onCancelImport) { Text(stringResource(R.string.action_cancel)) } },
        )
    }
}
