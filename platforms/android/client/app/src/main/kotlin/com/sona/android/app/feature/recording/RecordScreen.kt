package com.sona.android.app.feature.recording

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.pm.PackageManager
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.app.ui.theme.LocalSonaRecordingColor
import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.StreamingStatus
import com.sona.android.application.recording.TranscriptSegment

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RecordScreen(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    credentialStatus: CredentialStatus,
    onRetryBootstrap: () -> Unit,
    onStartRecording: () -> Unit,
    onStopRecording: () -> Unit,
    onConfigureCredential: () -> Unit,
) {
    val context = LocalContext.current
    val activity = remember(context) { context.findActivity() }
    val lifecycleOwner = LocalLifecycleOwner.current
    var hasRequestedPermission by rememberSaveable { mutableStateOf(false) }
    var permissionRevision by remember { mutableIntStateOf(0) }
    var permissionIssue by remember { mutableStateOf<MicrophonePermissionDecision?>(null) }
    var displayedSegments by remember { mutableStateOf(emptyList<TranscriptSegment>()) }

    val permissionGranted = remember(permissionRevision, context) {
        context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
    }
    val shouldShowRationale = remember(permissionRevision, activity) {
        activity?.shouldShowRequestPermissionRationale(Manifest.permission.RECORD_AUDIO) == true
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        hasRequestedPermission = true
        permissionRevision += 1
        if (granted) {
            permissionIssue = null
            onStartRecording()
        } else {
            permissionIssue = MicrophonePermissionPolicy.decide(
                isGranted = false,
                hasRequestedBefore = true,
                shouldShowRationale =
                    activity?.shouldShowRequestPermissionRationale(
                        Manifest.permission.RECORD_AUDIO,
                    ) == true,
            )
        }
    }

    DisposableEffect(lifecycleOwner, context) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                permissionRevision += 1
                if (
                    context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
                    PackageManager.PERMISSION_GRANTED
                ) {
                    permissionIssue = null
                }
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(recordingState) {
        when (recordingState) {
            is LiveRecordingState.Preparing -> displayedSegments = emptyList()
            is LiveRecordingState.Recording -> displayedSegments = recordingState.segments
            else -> Unit
        }
    }

    val presentation = recordingState.toRecordingPresentation()
    val bootstrapReady = bootstrapState is SonaBootstrapUiState.Ready &&
        bootstrapState.snapshot.onlineStreamingAvailable
    val elapsedMillis = (recordingState as? LiveRecordingState.Recording)?.elapsedMillis ?: 0

    val requestRecording = {
        if (
            credentialStatus == CredentialStatus.NOT_CONFIGURED ||
            recordingState is LiveRecordingState.NeedsConfiguration
        ) {
            onConfigureCredential()
        } else {
            when (
                MicrophonePermissionPolicy.decide(
                    isGranted = permissionGranted,
                    hasRequestedBefore = hasRequestedPermission,
                    shouldShowRationale = shouldShowRationale,
                )
            ) {
                MicrophonePermissionDecision.START_RECORDING -> onStartRecording()
                MicrophonePermissionDecision.REQUEST_PERMISSION -> {
                    hasRequestedPermission = true
                    permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                }
                MicrophonePermissionDecision.SHOW_RATIONALE -> {
                    permissionIssue = MicrophonePermissionDecision.SHOW_RATIONALE
                }
                MicrophonePermissionDecision.OPEN_APP_SETTINGS -> {
                    permissionIssue = MicrophonePermissionDecision.OPEN_APP_SETTINGS
                }
            }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = stringResource(R.string.record_heading),
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.primary,
            fontWeight = FontWeight.SemiBold,
        )
        BootstrapStatus(bootstrapState, onRetryBootstrap)

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.engine_label),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
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
        }

        RecordingNotices(
            state = recordingState,
            permissionIssue = permissionIssue,
            onRetryPermission = {
                hasRequestedPermission = true
                permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
            },
            onOpenAppSettings = { context.openAppSettings() },
            onConfigureCredential = onConfigureCredential,
        )

        TranscriptList(
            segments = displayedSegments,
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f),
        )

        RecordControls(
            elapsedMillis = elapsedMillis,
            state = recordingState,
            presentation = presentation,
            bootstrapReady = bootstrapReady,
            onStart = requestRecording,
            onStop = onStopRecording,
        )
    }
}

@Composable
private fun TranscriptList(
    segments: List<TranscriptSegment>,
    modifier: Modifier = Modifier,
) {
    val listState = androidx.compose.foundation.lazy.rememberLazyListState()
    val lastSegment = segments.lastOrNull()

    LaunchedEffect(lastSegment?.id, lastSegment?.text) {
        if (segments.isNotEmpty()) {
            listState.animateScrollToItem(segments.lastIndex)
        }
    }

    LazyColumn(
        state = listState,
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(12.dp),
        contentPadding = PaddingValues(vertical = 8.dp)
    ) {
        if (segments.isEmpty()) {
            item {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(160.dp)
                        .background(
                            color = MaterialTheme.colorScheme.surfaceContainerLow,
                            shape = MaterialTheme.shapes.medium
                        ),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Mic,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                            modifier = Modifier.size(36.dp)
                        )
                        Text(
                            text = stringResource(R.string.transcript_waiting),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        } else {
            items(segments, key = TranscriptSegment::id) { segment ->
                val containerColor = if (segment.isFinal) {
                    MaterialTheme.colorScheme.surfaceContainer
                } else {
                    MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.7f)
                }
                val contentColor = if (segment.isFinal) {
                    MaterialTheme.colorScheme.onSurface
                } else {
                    MaterialTheme.colorScheme.onPrimaryContainer
                }

                Card(
                    shape = MaterialTheme.shapes.medium,
                    colors = CardDefaults.cardColors(
                        containerColor = containerColor,
                        contentColor = contentColor
                    ),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)
                    ) {
                        segment.speaker?.label?.takeIf(String::isNotBlank)?.let { speaker ->
                            Text(
                                text = speaker,
                                style = MaterialTheme.typography.labelMedium,
                                color = if (segment.isFinal) {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.8f)
                                },
                                fontWeight = FontWeight.SemiBold,
                                modifier = Modifier.padding(bottom = 4.dp)
                            )
                        }
                        Text(
                            text = segment.text,
                            style = MaterialTheme.typography.bodyLarge,
                            fontWeight = if (segment.isFinal) FontWeight.Normal else FontWeight.Medium,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun RecordingNotices(
    state: LiveRecordingState,
    permissionIssue: MicrophonePermissionDecision?,
    onRetryPermission: () -> Unit,
    onOpenAppSettings: () -> Unit,
    onConfigureCredential: () -> Unit,
) {
    val presentation = state.toRecordingPresentation()
    val failure = presentation.statusCategory.isFailure()
    NoticeRow(
        text = stringResource(presentation.statusCategory.labelRes()),
        isWarning = failure ||
            presentation.statusCategory == RecordingStatusCategory.COMPLETED_WITH_WARNING,
    )

    if (state is LiveRecordingState.NeedsConfiguration) {
        Spacer(Modifier.height(4.dp))
        FilledTonalButton(
            onClick = onConfigureCredential,
            modifier = Modifier.fillMaxWidth()
        ) {
            Text(stringResource(R.string.action_configure_credential))
        }
    }

    if (state is LiveRecordingState.Recording) {
        if (state.streamingStatus is StreamingStatus.AudioOnly) {
            NoticeRow(
                text = stringResource(R.string.recording_audio_only),
                isWarning = true,
            )
        }
        when (val inputStatus = state.inputStatus) {
            AudioInputStatus.Active -> Unit
            AudioInputStatus.Silenced -> NoticeRow(
                text = stringResource(R.string.microphone_silenced),
                isWarning = true,
            )
            AudioInputStatus.MonitoringUnavailable -> NoticeRow(
                text = stringResource(R.string.microphone_monitoring_unavailable),
            )
            is AudioInputStatus.DeviceChanged -> NoticeRow(
                text = inputStatus.deviceName?.takeIf(String::isNotBlank)?.let { name ->
                    stringResource(R.string.microphone_device_changed, name)
                } ?: stringResource(R.string.microphone_device_changed_unknown),
            )
        }
    }

    when (permissionIssue) {
        MicrophonePermissionDecision.SHOW_RATIONALE -> {
            NoticeRow(
                text = stringResource(R.string.microphone_permission_rationale),
                isWarning = true,
            )
            Spacer(Modifier.height(4.dp))
            FilledTonalButton(
                onClick = onRetryPermission,
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(stringResource(R.string.action_grant_permission))
            }
        }
        MicrophonePermissionDecision.OPEN_APP_SETTINGS -> {
            NoticeRow(
                text = stringResource(R.string.microphone_permission_settings),
                isWarning = true,
            )
            Spacer(Modifier.height(4.dp))
            FilledTonalButton(
                onClick = onOpenAppSettings,
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(stringResource(R.string.action_open_app_settings))
            }
        }
        MicrophonePermissionDecision.START_RECORDING,
        MicrophonePermissionDecision.REQUEST_PERMISSION,
        null,
        -> Unit
    }
}

@Composable
private fun NoticeRow(
    text: String,
    isWarning: Boolean = false,
) {
    if (isWarning) {
        Card(
            shape = MaterialTheme.shapes.small,
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.errorContainer,
                contentColor = MaterialTheme.colorScheme.onErrorContainer
            ),
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp)
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    imageVector = Icons.Rounded.WarningAmber,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(12.dp))
                Text(
                    text = text,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium
                )
            }
        }
    } else {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp, horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = Icons.Rounded.CheckCircle,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary.copy(alpha = 0.8f),
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(10.dp))
            Text(
                text = text,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun RecordControls(
    elapsedMillis: Long,
    state: LiveRecordingState,
    presentation: RecordingPresentation,
    bootstrapReady: Boolean,
    onStart: () -> Unit,
    onStop: () -> Unit,
) {
    val recording = state is LiveRecordingState.Recording
    val recordingColor = LocalSonaRecordingColor.current

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = formatRecordingTimer(elapsedMillis),
            style = MaterialTheme.typography.displayMedium,
            fontWeight = FontWeight.SemiBold,
            color = if (recording) recordingColor else MaterialTheme.colorScheme.onSurface
        )
        Spacer(Modifier.height(20.dp))
        if (state is LiveRecordingState.Preparing || state is LiveRecordingState.Stopping) {
            Box(
                modifier = Modifier.size(96.dp),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(Modifier.size(44.dp))
            }
        } else {
            val infiniteTransition = rememberInfiniteTransition(label = "pulse")
            val pulseScale by if (recording) {
                infiniteTransition.animateFloat(
                    initialValue = 1.0f,
                    targetValue = 1.35f,
                    animationSpec = infiniteRepeatable(
                        animation = tween(1200),
                        repeatMode = RepeatMode.Reverse
                    ),
                    label = "scale"
                )
            } else {
                remember { mutableStateOf(1.0f) }
            }

            val pulseAlpha by if (recording) {
                infiniteTransition.animateFloat(
                    initialValue = 0.45f,
                    targetValue = 0.0f,
                    animationSpec = infiniteRepeatable(
                        animation = tween(1200),
                        repeatMode = RepeatMode.Reverse
                    ),
                    label = "alpha"
                )
            } else {
                remember { mutableStateOf(0.0f) }
            }

            val enabled = if (recording) {
                presentation.isStopAvailable
            } else {
                presentation.isStartAvailable && bootstrapReady
            }

            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier.size(110.dp)
            ) {
                if (recording) {
                    Box(
                        modifier = Modifier
                            .size(76.dp)
                            .scale(pulseScale)
                            .background(
                                color = recordingColor.copy(alpha = pulseAlpha),
                                shape = CircleShape
                            )
                    )
                }

                FloatingActionButton(
                    onClick = {
                        if (enabled) {
                            if (recording) onStop() else onStart()
                        }
                    },
                    shape = CircleShape,
                    containerColor = if (!enabled) {
                        MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f)
                    } else if (recording) {
                        recordingColor
                    } else {
                        MaterialTheme.colorScheme.primaryContainer
                    },
                    contentColor = if (!enabled) {
                        MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                    } else if (recording) {
                        Color.White
                    } else {
                        MaterialTheme.colorScheme.onPrimaryContainer
                    },
                    elevation = FloatingActionButtonDefaults.elevation(
                        defaultElevation = if (enabled) 4.dp else 0.dp,
                        pressedElevation = if (enabled) 2.dp else 0.dp
                    ),
                    modifier = Modifier.size(76.dp)
                ) {
                    Icon(
                        imageVector = if (recording) Icons.Rounded.Stop else Icons.Rounded.Mic,
                        contentDescription = stringResource(
                            if (recording) {
                                R.string.record_stop_description
                            } else {
                                R.string.record_action_description
                            }
                        ),
                        modifier = Modifier.size(34.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun BootstrapStatus(
    bootstrapState: SonaBootstrapUiState,
    onRetryBootstrap: () -> Unit,
) {
    Card(
        shape = MaterialTheme.shapes.small,
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = stringResource(R.string.engine_label) + ": ",
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.width(4.dp))
            when (bootstrapState) {
                SonaBootstrapUiState.Loading -> {
                    CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = stringResource(R.string.status_loading),
                        style = MaterialTheme.typography.bodyMedium
                    )
                }
                is SonaBootstrapUiState.Ready -> {
                    Icon(
                        imageVector = Icons.Rounded.CheckCircle,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(16.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = stringResource(R.string.status_ready),
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium
                    )
                }
                is SonaBootstrapUiState.Error -> {
                    Text(
                        text = stringResource(R.string.status_error),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium
                    )
                    Spacer(Modifier.weight(1f))
                    FilledTonalButton(
                        onClick = onRetryBootstrap,
                        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 2.dp),
                        modifier = Modifier.height(30.dp)
                    ) {
                        Text(
                            text = stringResource(R.string.action_retry),
                            style = MaterialTheme.typography.labelMedium
                        )
                    }
                }
            }
        }
    }
}

@StringRes
private fun RecordingStatusCategory.labelRes(): Int = when (this) {
    RecordingStatusCategory.IDLE -> R.string.recording_idle
    RecordingStatusCategory.NEEDS_CONFIGURATION -> R.string.recording_needs_configuration
    RecordingStatusCategory.PREPARING -> R.string.recording_preparing
    RecordingStatusCategory.RECORDING -> R.string.recording_active
    RecordingStatusCategory.STOPPING -> R.string.recording_stopping
    RecordingStatusCategory.COMPLETED -> R.string.recording_completed
    RecordingStatusCategory.COMPLETED_WITH_WARNING -> R.string.recording_completed_warning
    RecordingStatusCategory.INVALID_CONFIGURATION_FAILURE ->
        R.string.recording_invalid_configuration
    RecordingStatusCategory.STARTUP_FAILURE -> R.string.recording_startup_failed
    RecordingStatusCategory.AUDIO_FAILURE -> R.string.recording_audio_failed
    RecordingStatusCategory.STREAMING_FAILURE -> R.string.recording_streaming_failed
    RecordingStatusCategory.PERSISTENCE_FAILURE -> R.string.recording_persistence_failed
}

private fun RecordingStatusCategory.isFailure(): Boolean = when (this) {
    RecordingStatusCategory.INVALID_CONFIGURATION_FAILURE,
    RecordingStatusCategory.STARTUP_FAILURE,
    RecordingStatusCategory.AUDIO_FAILURE,
    RecordingStatusCategory.STREAMING_FAILURE,
    RecordingStatusCategory.PERSISTENCE_FAILURE,
    -> true
    else -> false
}

private fun Context.findActivity(): Activity? {
    var current = this
    while (current is ContextWrapper) {
        if (current is Activity) {
            return current
        }
        current = current.baseContext
    }
    return current as? Activity
}

private fun Context.openAppSettings() {
    startActivity(
        Intent(
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            "package:$packageName".toUri(),
        ),
    )
}
