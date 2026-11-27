package com.sona.android.app.feature.settings

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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.ui.component.InsetGroupedCard
import com.sona.android.app.ui.component.springOverscroll
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoveryStage
import com.sona.android.application.recovery.RecoveryUnavailableReason

@OptIn(ExperimentalMaterial3Api::class)
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
    extraBottomPadding: Dp = 0.dp,
) {
    val backupExporter = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/x-bzip2"),
    ) { uri -> uri?.let { onExportBackup(it.toString()) } }
    val backupImporter = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri -> uri?.let { onInspectBackup(it.toString()) } }
    val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()

    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxWidth()
                .springOverscroll()
                .verticalScroll(rememberScrollState())
                .padding(
                    start = 24.dp,
                    end = 24.dp,
                    top = 20.dp,
                    bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
                )
                .align(Alignment.TopCenter),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            val errorText = when (state.errorCategory) {
                DataRecoveryErrorCategory.BLOCKED -> stringResource(R.string.backup_blocked)
                DataRecoveryErrorCategory.OPERATION_FAILED -> stringResource(R.string.data_operation_failed)
                null -> state.error
            }
            if (errorText != null) {
                Surface(
                    shape = RoundedCornerShape(18.dp),
                    color = MaterialTheme.colorScheme.errorContainer,
                    contentColor = MaterialTheme.colorScheme.onErrorContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = errorText,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(14.dp),
                    )
                }
            }

            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = stringResource(R.string.backup_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    if (state.blockers.isNotEmpty()) {
                        Text(
                            text = stringResource(R.string.backup_blocked),
                            color = MaterialTheme.colorScheme.tertiary,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Button(
                            onClick = { backupExporter.launch("sona-backup.tar.bz2") },
                            enabled = !state.busy && state.blockers.isEmpty(),
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(stringResource(R.string.backup_export))
                        }
                        OutlinedButton(
                            onClick = { backupImporter.launch(arrayOf("application/x-bzip2", "application/octet-stream")) },
                            enabled = !state.busy && state.blockers.isEmpty(),
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(stringResource(R.string.backup_import))
                        }
                    }
                    Text(
                        text = stringResource(R.string.backup_excludes),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = stringResource(R.string.recovery_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    if (state.recovery.items.any { it.resolution == RecoveryResolution.PENDING && it.canResume }) {
                        Button(
                            onClick = onResumeAll,
                            enabled = !state.busy,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text(stringResource(R.string.recovery_resume_all))
                        }
                    }
                    state.recovery.items.forEach { item ->
                        Surface(
                            shape = RoundedCornerShape(14.dp),
                            color = MaterialTheme.colorScheme.surfaceContainerLow,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Column(
                                modifier = Modifier.padding(12.dp),
                                verticalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                Text(
                                    text = item.filename,
                                    style = MaterialTheme.typography.titleSmall,
                                    fontWeight = FontWeight.SemiBold,
                                )
                                Text(
                                    text = stringResource(
                                        R.string.recovery_stage_progress,
                                        stringResource(item.stage.labelRes()),
                                        (item.progress * 100).toInt(),
                                    ),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
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
                                if (item.resolution == RecoveryResolution.PENDING) {
                                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                        Button(onClick = { onResume(item.id) }, enabled = item.canResume && !state.busy) {
                                            Text(stringResource(R.string.recovery_resume))
                                        }
                                        OutlinedButton(onClick = { onDiscard(item.id) }, enabled = !state.busy) {
                                            Text(stringResource(R.string.recovery_discard))
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        OutlinedButton(
                            onClick = onRefresh,
                            enabled = !state.busy,
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(stringResource(R.string.action_refresh))
                        }
                        OutlinedButton(
                            onClick = onClearResolved,
                            enabled = !state.busy,
                            modifier = Modifier.weight(1f),
                        ) {
                            Text(stringResource(R.string.recovery_clear_resolved))
                        }
                    }
                }
            }
        }
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
                        prepared.manifest.counts.projects,
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

@StringRes
private fun RecoveryStage.labelRes(): Int = when (this) {
    RecoveryStage.QUEUED -> R.string.recovery_stage_queued
    RecoveryStage.TRANSCODING -> R.string.recovery_stage_transcoding
    RecoveryStage.TRANSCRIBING -> R.string.recovery_stage_transcribing
    RecoveryStage.SAVING -> R.string.recovery_stage_saving
    RecoveryStage.EXPORTING -> R.string.recovery_stage_exporting
}
