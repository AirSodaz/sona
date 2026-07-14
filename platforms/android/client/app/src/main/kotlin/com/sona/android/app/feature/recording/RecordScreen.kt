package com.sona.android.app.feature.recording

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.StreamingStatus
import com.sona.android.application.recording.TranscriptSegment
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RecordScreen(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    microphonePermissionGranted: Boolean,
    onRecordAction: () -> Unit,
    onOpenSettings: () -> Unit,
    onRetryBootstrap: () -> Unit,
) {
    val onlineAvailable = (bootstrapState as? SonaBootstrapUiState.Ready)
        ?.snapshot?.onlineStreamingAvailable == true
    val actionEnabled = onlineAvailable &&
        recordingState !is LiveRecordingState.Preparing &&
        recordingState !is LiveRecordingState.Stopping
    val isRecording = recordingState is LiveRecordingState.Recording
    val segments = (recordingState as? LiveRecordingState.Recording)?.segments.orEmpty()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 20.dp),
    ) {
        Text(
            text = stringResource(R.string.record_heading),
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(8.dp))
        BootstrapStatus(bootstrapState, onRetryBootstrap)
        Spacer(Modifier.height(20.dp))
        HorizontalDivider()
        Spacer(Modifier.height(16.dp))
        Text(
            text = stringResource(R.string.engine_label),
            style = MaterialTheme.typography.labelLarge,
        )
        Spacer(Modifier.height(10.dp))
        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            SegmentedButton(
                selected = true,
                onClick = {},
                shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
                label = { Text(stringResource(R.string.engine_online)) },
            )
            SegmentedButton(
                selected = false,
                onClick = {},
                enabled = false,
                shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
                label = { Text(stringResource(R.string.engine_local)) },
            )
        }
        Spacer(Modifier.height(16.dp))
        TranscriptSurface(
            segments = segments,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
        )
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = formatElapsedMillis(
                    (recordingState as? LiveRecordingState.Recording)?.elapsedMillis ?: 0,
                ),
                style = MaterialTheme.typography.displaySmall,
            )
            Spacer(Modifier.height(16.dp))
            FilledIconButton(
                onClick = onRecordAction,
                enabled = actionEnabled,
                colors = if (isRecording) {
                    IconButtonDefaults.filledIconButtonColors(
                        containerColor = MaterialTheme.colorScheme.error,
                        contentColor = MaterialTheme.colorScheme.onError,
                    )
                } else {
                    IconButtonDefaults.filledIconButtonColors()
                },
                modifier = Modifier.size(88.dp),
            ) {
                Icon(
                    imageVector = if (isRecording) Icons.Rounded.Stop else Icons.Rounded.Mic,
                    contentDescription = stringResource(
                        if (isRecording) {
                            R.string.stop_recording_action_description
                        } else {
                            R.string.record_action_description
                        },
                    ),
                    modifier = Modifier.size(34.dp),
                )
            }
            Spacer(Modifier.height(10.dp))
            Text(
                text = recordingStatus(
                    state = recordingState,
                    onlineAvailable = onlineAvailable,
                    microphonePermissionGranted = microphonePermissionGranted,
                ),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (recordingState is LiveRecordingState.NeedsConfiguration) {
                TextButton(onClick = onOpenSettings) {
                    Icon(
                        imageVector = Icons.Rounded.Settings,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.action_open_settings))
                }
            } else {
                Spacer(Modifier.height(12.dp))
            }
        }
    }
}

@Composable
private fun TranscriptSurface(
    segments: List<TranscriptSegment>,
    modifier: Modifier,
) {
    Box(modifier = modifier) {
        if (segments.isNotEmpty()) {
            LazyColumn(modifier = Modifier.fillMaxSize()) {
                items(segments, key = TranscriptSegment::id) { segment ->
                    Text(
                        text = segment.text,
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (segment.isFinal) {
                            MaterialTheme.colorScheme.onSurface
                        } else {
                            MaterialTheme.colorScheme.primary
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 8.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun recordingStatus(
    state: LiveRecordingState,
    onlineAvailable: Boolean,
    microphonePermissionGranted: Boolean,
): String {
    if (!onlineAvailable) return stringResource(R.string.status_online_unavailable)
    if (!microphonePermissionGranted && state !is LiveRecordingState.Recording) {
        return stringResource(R.string.status_microphone_permission)
    }
    return when (state) {
        LiveRecordingState.Idle -> stringResource(R.string.status_ready)
        LiveRecordingState.NeedsConfiguration ->
            stringResource(R.string.status_credentials_required)
        is LiveRecordingState.Preparing -> stringResource(R.string.status_preparing)
        is LiveRecordingState.Recording -> when {
            state.streamingStatus is StreamingStatus.AudioOnly ->
                stringResource(R.string.status_audio_only)
            state.inputStatus is AudioInputStatus.Silenced ->
                stringResource(R.string.status_input_silenced)
            else -> stringResource(R.string.status_recording)
        }
        is LiveRecordingState.Stopping -> stringResource(R.string.status_stopping)
        is LiveRecordingState.Completed -> if (state.warning == null) {
            stringResource(R.string.status_saved)
        } else {
            stringResource(R.string.status_saved_with_warning)
        }
        is LiveRecordingState.Failed -> stringResource(R.string.status_recording_failed)
    }
}

internal fun formatElapsedMillis(elapsedMillis: Long): String {
    val totalSeconds = elapsedMillis.coerceAtLeast(0) / 1_000
    val hours = totalSeconds / 3_600
    val minutes = (totalSeconds % 3_600) / 60
    val seconds = totalSeconds % 60
    return if (hours > 0) {
        String.format(Locale.ROOT, "%02d:%02d:%02d", hours, minutes, seconds)
    } else {
        String.format(Locale.ROOT, "%02d:%02d", minutes, seconds)
    }
}

@Composable
private fun BootstrapStatus(
    bootstrapState: SonaBootstrapUiState,
    onRetryBootstrap: () -> Unit,
) {
    when (bootstrapState) {
        SonaBootstrapUiState.Loading -> Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            Spacer(Modifier.width(10.dp))
            Text(stringResource(R.string.status_loading))
        }

        is SonaBootstrapUiState.Ready -> Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = Icons.Rounded.CheckCircle,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(10.dp))
            Text(stringResource(R.string.status_ready))
        }

        is SonaBootstrapUiState.Error -> Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.status_error),
                color = MaterialTheme.colorScheme.error,
            )
            Spacer(Modifier.width(8.dp))
            TextButton(onClick = onRetryBootstrap) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}
