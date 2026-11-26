package com.sona.android.app.feature.recording

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.app.notification.SonaNotificationChannels
import com.sona.android.app.ui.component.SonaWaveform
import com.sona.android.app.ui.theme.LocalSonaRecordingColor
import com.sona.android.app.ui.theme.LocalSonaRecordingContainerColor
import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AudioInputStatus
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.StreamingStatus
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private const val POST_NOTIFICATIONS_PERMISSION = "android.permission.POST_NOTIFICATIONS"
private const val WAVEFORM_HISTORY_SIZE = 36

/**
 * Persistent floating mini-bar displayed above the dock when a live recording
 * is running in the background while the user browses the library or settings.
 */
@Composable
internal fun RecordMiniBar(
    recordingState: LiveRecordingState,
    onExpand: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val recordingColor = LocalSonaRecordingColor.current
    val isRecording = recordingState is LiveRecordingState.Recording
    val isPaused = (recordingState as? LiveRecordingState.Recording)?.isPaused == true
    val isBusy = recordingState is LiveRecordingState.Preparing ||
        recordingState is LiveRecordingState.Stopping
    val elapsedMillis = (recordingState as? LiveRecordingState.Recording)?.elapsedMillis ?: 0L
    val latestText = (recordingState as? LiveRecordingState.Recording)?.segments
        ?.lastOrNull()?.text?.takeIf { it.isNotBlank() }

    val pulseAlpha = if (isRecording && !isPaused) {
        rememberInfiniteTransition(label = "miniBarPulse").animateFloat(
            initialValue = 1f,
            targetValue = 0.28f,
            animationSpec = infiniteRepeatable(
                animation = tween(680),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "miniBarPulseAlpha",
        ).value
    } else {
        0.7f
    }

    Surface(
        modifier = modifier
            .widthIn(max = 480.dp)
            .fillMaxWidth(),
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
        tonalElevation = 4.dp,
        shadowElevation = 6.dp,
        border = BorderStroke(
            width = 0.8.dp,
            color = recordingColor.copy(alpha = 0.3f),
        ),
        onClick = onExpand,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (isBusy) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                )
            } else {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .alpha(pulseAlpha)
                        .background(
                            color = if (isPaused) MaterialTheme.colorScheme.tertiary else recordingColor,
                            shape = CircleShape,
                        ),
                )
            }

            Column(modifier = Modifier.weight(1f)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        text = formatRecordingTimer(elapsedMillis),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                        color = if (isRecording && !isPaused) recordingColor else MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = stringResource(
                            when {
                                recordingState is LiveRecordingState.Preparing -> R.string.recording_preparing
                                recordingState is LiveRecordingState.Stopping -> R.string.recording_stopping
                                isPaused -> R.string.recording_paused
                                else -> R.string.recording_active
                            },
                        ),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                if (latestText != null) {
                    Text(
                        text = latestText,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            if (isRecording) {
                FilledTonalIconButton(
                    onClick = { if (isPaused) onResume() else onPause() },
                    modifier = Modifier.size(36.dp),
                ) {
                    Icon(
                        imageVector = if (isPaused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause,
                        contentDescription = stringResource(
                            if (isPaused) R.string.record_resume_description else R.string.record_pause_description,
                        ),
                        modifier = Modifier.size(18.dp),
                    )
                }
                FilledTonalIconButton(
                    onClick = onStop,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = LocalSonaRecordingContainerColor.current,
                        contentColor = recordingColor,
                    ),
                    modifier = Modifier.size(36.dp),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Stop,
                        contentDescription = stringResource(R.string.record_stop_description),
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RecordScreen(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    configuredProviders: Set<OnlineAsrProvider>,
    recognitionSettings: RecognitionSettingsUiState,
    autoStartRequestToken: Int = 0,
    onRetryBootstrap: () -> Unit,
    onStartRecording: () -> Unit,
    onPauseRecording: () -> Unit = {},
    onResumeRecording: () -> Unit = {},
    onStopRecording: () -> Unit,
    onSelectLiveModel: (AsrModelSelection?) -> Unit = {},
    onConfigureCredential: () -> Unit,
    onConfigureRecognition: () -> Unit,
    onViewCompleted: (String) -> Unit = {},
    onMinimize: () -> Unit = {},
) {
    BackHandler(onBack = onMinimize)

    val context = LocalContext.current
    val activity = remember(context) { context.findActivity() }
    val lifecycleOwner = LocalLifecycleOwner.current

    var hasRequestedPermission by rememberSaveable { mutableStateOf(false) }
    var hasRequestedNotificationPermission by rememberSaveable { mutableStateOf(false) }
    var permissionRevision by remember { mutableIntStateOf(0) }
    var permissionIssue by remember { mutableStateOf<MicrophonePermissionDecision?>(null) }
    var displayedSegments by remember { mutableStateOf(emptyList<TranscriptSegment>()) }
    var engineSheetVisible by remember { mutableStateOf(false) }
    var setupSheetVisible by remember { mutableStateOf(false) }

    val waveformBars = remember {
        mutableStateListOf<Float>().apply {
            repeat(WAVEFORM_HISTORY_SIZE) { add(0f) }
        }
    }

    val permissionGranted = remember(permissionRevision, context) {
        context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
    }
    val shouldShowRationale = remember(permissionRevision, activity) {
        activity?.shouldShowRequestPermissionRationale(Manifest.permission.RECORD_AUDIO) == true
    }
    val shouldShowNotificationRationale = remember(permissionRevision, activity) {
        activity?.shouldShowRequestPermissionRationale(POST_NOTIFICATIONS_PERMISSION) == true
    }
    val notificationPermissionGranted = remember(permissionRevision, context) {
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            context.checkSelfPermission(POST_NOTIFICATIONS_PERMISSION) ==
            PackageManager.PERMISSION_GRANTED
    }

    val notificationPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) {
        hasRequestedNotificationPermission = true
        permissionRevision += 1
        onStartRecording()
    }
    val startAfterOptionalNotificationPermission = {
        when (
            NotificationPermissionPolicy.decide(
                requiresRuntimePermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU,
                isGranted = notificationPermissionGranted,
                hasRequestedBefore = hasRequestedNotificationPermission,
            )
        ) {
            NotificationPermissionDecision.START_RECORDING -> onStartRecording()
            NotificationPermissionDecision.REQUEST_PERMISSION -> {
                hasRequestedNotificationPermission = true
                notificationPermissionLauncher.launch(POST_NOTIFICATIONS_PERMISSION)
            }
        }
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        hasRequestedPermission = true
        permissionRevision += 1
        if (granted) {
            permissionIssue = null
            setupSheetVisible = false
            startAfterOptionalNotificationPermission()
        } else {
            permissionIssue = MicrophonePermissionPolicy.decide(
                isGranted = false,
                hasRequestedBefore = true,
                shouldShowRationale =
                    activity?.shouldShowRequestPermissionRationale(
                        Manifest.permission.RECORD_AUDIO,
                    ) == true,
            )
            setupSheetVisible = true
        }
    }

    DisposableEffect(lifecycleOwner, context) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                permissionRevision += 1
                val isGranted = context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
                    PackageManager.PERMISSION_GRANTED
                if (isGranted) {
                    permissionIssue = null
                } else if (hasRequestedPermission) {
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
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(recordingState) {
        when (recordingState) {
            is LiveRecordingState.Preparing -> {
                displayedSegments = emptyList()
                for (i in waveformBars.indices) waveformBars[i] = 0f
            }
            is LiveRecordingState.Recording -> {
                displayedSegments = recordingState.segments
                if (!recordingState.isPaused) {
                    if (waveformBars.size >= WAVEFORM_HISTORY_SIZE) {
                        waveformBars.removeAt(0)
                    }
                    waveformBars.add(recordingState.rmsLevel)
                }
            }
            else -> Unit
        }
    }

    // Gentle live decay ticks while recording without new frames
    LaunchedEffect(
        recordingState is LiveRecordingState.Recording && !recordingState.isPaused,
    ) {
        val active = recordingState is LiveRecordingState.Recording && !recordingState.isPaused
        while (active) {
            delay(85)
            val currentLevel = (recordingState as? LiveRecordingState.Recording)?.rmsLevel ?: 0f
            if (waveformBars.size >= WAVEFORM_HISTORY_SIZE) {
                waveformBars.removeAt(0)
            }
            waveformBars.add(currentLevel)
        }
    }

    val presentation = recordingState.toRecordingPresentation()
    val liveSelection = recognitionSettings.liveSelection
    val onlineCredentialConfigured = OnlineAsrProvider.VOLCENGINE_DOUBAO in configuredProviders
    val bootstrapReady = bootstrapState is SonaBootstrapUiState.Ready && when (liveSelection) {
        is AsrModelSelection.Online -> bootstrapState.snapshot.onlineStreamingAvailable
        is AsrModelSelection.Local -> bootstrapState.snapshot.localStreamingSessionAvailable
        null -> false
    }
    val configurationMissing = when (liveSelection) {
        is AsrModelSelection.Online -> !onlineCredentialConfigured
        is AsrModelSelection.Local -> recognitionSettings.installedModels.none {
            it.id == liveSelection.modelId
        }
        null -> true
    }
    val elapsedMillis = (recordingState as? LiveRecordingState.Recording)?.elapsedMillis ?: 0L
    val isActive = recordingState is LiveRecordingState.Preparing ||
        recordingState is LiveRecordingState.Recording ||
        recordingState is LiveRecordingState.Stopping

    val requestRecording = {
        if (configurationMissing || recordingState is LiveRecordingState.NeedsConfiguration) {
            setupSheetVisible = true
        } else if (!bootstrapReady) {
            setupSheetVisible = true
        } else {
            when (
                MicrophonePermissionPolicy.decide(
                    isGranted = permissionGranted,
                    hasRequestedBefore = hasRequestedPermission,
                    shouldShowRationale = shouldShowRationale,
                )
            ) {
                MicrophonePermissionDecision.START_RECORDING ->
                    startAfterOptionalNotificationPermission()
                MicrophonePermissionDecision.REQUEST_PERMISSION -> {
                    hasRequestedPermission = true
                    permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                }
                MicrophonePermissionDecision.SHOW_RATIONALE -> {
                    permissionIssue = MicrophonePermissionDecision.SHOW_RATIONALE
                    setupSheetVisible = true
                }
                MicrophonePermissionDecision.OPEN_APP_SETTINGS -> {
                    permissionIssue = MicrophonePermissionDecision.OPEN_APP_SETTINGS
                    setupSheetVisible = true
                }
            }
        }
    }

    LaunchedEffect(autoStartRequestToken) {
        if (autoStartRequestToken > 0 && !isActive) {
            requestRecording()
        }
    }

    if (engineSheetVisible) {
        LiveEngineSelectorSheet(
            recognitionSettings = recognitionSettings,
            configuredProviders = configuredProviders,
            onSelectModel = { selection ->
                onSelectLiveModel(selection)
                engineSheetVisible = false
            },
            onOpenRecognitionSettings = {
                engineSheetVisible = false
                onConfigureRecognition()
            },
            onDismiss = { engineSheetVisible = false },
        )
    }

    if (setupSheetVisible) {
        RecordingSetupSheet(
            bootstrapState = bootstrapState,
            configurationMissing = configurationMissing,
            liveSelection = liveSelection,
            permissionIssue = permissionIssue,
            onRetryBootstrap = onRetryBootstrap,
            onRetryPermission = {
                setupSheetVisible = false
                hasRequestedPermission = true
                permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
            },
            onOpenAppSettings = {
                setupSheetVisible = false
                context.openAppSettings()
            },
            onConfigureCredential = {
                setupSheetVisible = false
                onConfigureCredential()
            },
            onConfigureRecognition = {
                setupSheetVisible = false
                onConfigureRecognition()
            },
            onSelectEngine = {
                setupSheetVisible = false
                engineSheetVisible = true
            },
            onDismiss = { setupSheetVisible = false },
        )
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding()
                .navigationBarsPadding(),
            contentAlignment = Alignment.TopCenter,
        ) {
            Column(
                modifier = Modifier
                    .widthIn(max = 720.dp)
                    .fillMaxSize(),
            ) {
                // 1. Minimalist Studio Top Bar
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    IconButton(onClick = onMinimize) {
                        Icon(
                            imageVector = Icons.Rounded.KeyboardArrowDown,
                            contentDescription = stringResource(R.string.action_back),
                            modifier = Modifier.size(28.dp),
                        )
                    }

                    // Active Engine Selector Pill
                    Surface(
                        onClick = { if (!isActive) engineSheetVisible = true },
                        shape = RoundedCornerShape(20.dp),
                        color = if (configurationMissing) {
                            MaterialTheme.colorScheme.errorContainer
                        } else {
                            MaterialTheme.colorScheme.surfaceContainerHigh
                        },
                        contentColor = if (configurationMissing) {
                            MaterialTheme.colorScheme.onErrorContainer
                        } else {
                            MaterialTheme.colorScheme.onSurface
                        },
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 14.dp, vertical = 7.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.GraphicEq,
                                contentDescription = null,
                                modifier = Modifier.size(16.dp),
                                tint = if (configurationMissing) {
                                    MaterialTheme.colorScheme.error
                                } else {
                                    MaterialTheme.colorScheme.primary
                                },
                            )
                            Text(
                                text = liveSelection.liveModelSummary(recognitionSettings),
                                style = MaterialTheme.typography.labelLarge,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            if (!isActive) {
                                Icon(
                                    imageVector = Icons.Rounded.ExpandMore,
                                    contentDescription = null,
                                    modifier = Modifier.size(16.dp),
                                )
                            }
                        }
                    }

                    // Balance spacer / Settings shortcut when idle
                    if (!isActive) {
                        IconButton(onClick = onConfigureRecognition) {
                            Icon(
                                imageVector = Icons.Rounded.Settings,
                                contentDescription = stringResource(R.string.action_configure),
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    } else {
                        Spacer(modifier = Modifier.size(48.dp))
                    }
                }

                // 2. Hero Waveform + Timer Zone
                val recordingColor = LocalSonaRecordingColor.current
                val isRecording = recordingState is LiveRecordingState.Recording
                val isPaused = (recordingState as? LiveRecordingState.Recording)?.isPaused == true

                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    // Status Pill
                    val statusCategory = recordingStatusForDisplay(recordingState, configurationMissing)
                    if (statusCategory != null) {
                        val isError = statusCategory.isFailure() ||
                            statusCategory == RecordingStatusCategory.NEEDS_CONFIGURATION
                        Surface(
                            shape = RoundedCornerShape(16.dp),
                            color = when {
                                isError -> MaterialTheme.colorScheme.errorContainer
                                isRecording && !isPaused -> LocalSonaRecordingContainerColor.current
                                isPaused -> MaterialTheme.colorScheme.tertiaryContainer
                                else -> MaterialTheme.colorScheme.secondaryContainer
                            },
                            contentColor = when {
                                isError -> MaterialTheme.colorScheme.onErrorContainer
                                isRecording && !isPaused -> recordingColor
                                isPaused -> MaterialTheme.colorScheme.onTertiaryContainer
                                else -> MaterialTheme.colorScheme.onSecondaryContainer
                            },
                        ) {
                            Text(
                                text = stringResource(statusCategory.labelRes()),
                                style = MaterialTheme.typography.labelMedium,
                                fontWeight = FontWeight.SemiBold,
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                            )
                        }
                        Spacer(Modifier.height(8.dp))
                    }

                    Text(
                        text = formatRecordingTimer(elapsedMillis),
                        style = MaterialTheme.typography.displayLarge,
                        color = if (isRecording && !isPaused) {
                            recordingColor
                        } else {
                            MaterialTheme.colorScheme.onSurface
                        },
                    )

                    Spacer(Modifier.height(14.dp))

                    SonaWaveform(
                        amplitudes = waveformBars,
                        active = isRecording && !isPaused,
                        activeColor = recordingColor,
                        inactiveColor = MaterialTheme.colorScheme.outlineVariant,
                        height = 56.dp,
                    )
                }

                // Notices (AudioOnly / Mic silenced / Notification rationale / Completed card)
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    BootstrapStatus(bootstrapState, onRetryBootstrap)
                    RecordingNotices(
                        state = recordingState,
                        configurationMissing = configurationMissing,
                        permissionIssue = permissionIssue,
                        onOpenSetup = { setupSheetVisible = true },
                        onViewCompleted = onViewCompleted,
                        showNotificationNotice = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                            !notificationPermissionGranted &&
                            hasRequestedNotificationPermission,
                        onOpenNotificationSettings = { SonaNotificationChannels.openNotificationSettings(context) },
                        shouldShowNotificationRationale = shouldShowNotificationRationale,
                    )
                }

                // 3. Flowing Live Transcript Stream
                FlowingTranscriptStream(
                    segments = displayedSegments,
                    isRecording = isRecording,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .padding(horizontal = 24.dp),
                )

                // 4. Expressive Bottom Control Deck
                RecordControlDeck(
                    state = recordingState,
                    presentation = presentation,
                    bootstrapReady = bootstrapReady && !configurationMissing,
                    onStart = requestRecording,
                    onPause = onPauseRecording,
                    onResume = onResumeRecording,
                    onStop = onStopRecording,
                )
            }
        }
    }
}

@Composable
internal fun AsrModelSelection?.liveModelSummary(state: RecognitionSettingsUiState): String =
    when (this) {
        is AsrModelSelection.Local -> state.installedModels.firstOrNull { it.id == modelId }
            ?.displayName
            ?: stringResource(R.string.local_model_not_installed)
        is AsrModelSelection.Online -> when (provider) {
            OnlineAsrProvider.VOLCENGINE_DOUBAO -> stringResource(R.string.batch_provider_volcengine_doubao)
            OnlineAsrProvider.GROQ_WHISPER -> stringResource(R.string.batch_provider_groq_whisper)
            OnlineAsrProvider.MISTRAL_VOXTRAL -> stringResource(R.string.batch_provider_mistral_voxtral)
        }
        null -> stringResource(R.string.local_model_not_installed)
    }

@Composable
private fun FlowingTranscriptStream(
    segments: List<TranscriptSegment>,
    isRecording: Boolean,
    modifier: Modifier = Modifier,
) {
    val listState = rememberLazyListState()
    val coroutineScope = rememberCoroutineScope()
    val lastSegment = segments.lastOrNull()

    val isAtBottom by remember {
        derivedStateOf {
            val layout = listState.layoutInfo
            val lastVisible = layout.visibleItemsInfo.lastOrNull()?.index ?: 0
            layout.totalItemsCount == 0 || lastVisible >= layout.totalItemsCount - 2
        }
    }

    LaunchedEffect(lastSegment?.id, lastSegment?.text) {
        if (segments.isNotEmpty() && isAtBottom) {
            listState.animateScrollToItem(segments.lastIndex)
        }
    }

    Box(modifier = modifier) {
        if (segments.isEmpty()) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Mic,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
                        modifier = Modifier.size(40.dp),
                    )
                    Text(
                        text = stringResource(
                            if (isRecording) R.string.transcript_waiting else R.string.recording_idle,
                        ),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        } else {
            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(14.dp),
                contentPadding = PaddingValues(vertical = 16.dp),
            ) {
                items(segments, key = TranscriptSegment::id) { segment ->
                    FlowingTranscriptSegmentRow(segment = segment)
                }
            }

            androidx.compose.animation.AnimatedVisibility(
                visible = !isAtBottom,
                enter = fadeIn(),
                exit = fadeOut(),
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 12.dp),
            ) {
                FilledTonalButton(
                    onClick = {
                        coroutineScope.launch {
                            listState.animateScrollToItem(segments.lastIndex)
                        }
                    },
                    shape = RoundedCornerShape(20.dp),
                ) {
                    Icon(
                        imageVector = Icons.Rounded.ArrowDownward,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp),
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(stringResource(R.string.record_jump_to_latest))
                }
            }
        }
    }
}

@Composable
private fun FlowingTranscriptSegmentRow(segment: TranscriptSegment) {
    val isPartial = !segment.isFinal
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .then(
                if (isPartial) {
                    Modifier
                        .background(
                            color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.48f),
                            shape = RoundedCornerShape(16.dp),
                        )
                        .padding(horizontal = 14.dp, vertical = 10.dp)
                } else {
                    Modifier.padding(horizontal = 4.dp, vertical = 2.dp)
                },
            ),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        segment.speaker?.label?.takeIf(String::isNotBlank)?.let { speaker ->
            Surface(
                shape = RoundedCornerShape(8.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
            ) {
                Text(
                    text = speaker,
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
                )
            }
        }
        Text(
            text = segment.text,
            style = MaterialTheme.typography.bodyLarge,
            color = if (isPartial) {
                MaterialTheme.colorScheme.onPrimaryContainer
            } else {
                MaterialTheme.colorScheme.onSurface
            },
            fontWeight = if (isPartial) FontWeight.Medium else FontWeight.Normal,
        )
    }
}

@Composable
private fun RecordControlDeck(
    state: LiveRecordingState,
    presentation: RecordingPresentation,
    bootstrapReady: Boolean,
    onStart: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
) {
    val recording = state is LiveRecordingState.Recording
    val isPaused = (state as? LiveRecordingState.Recording)?.isPaused == true
    val recordingColor = LocalSonaRecordingColor.current

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp, vertical = 16.dp),
        shape = RoundedCornerShape(32.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        tonalElevation = 2.dp,
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 14.dp),
            contentAlignment = Alignment.Center,
        ) {
            when {
                state is LiveRecordingState.Preparing || state is LiveRecordingState.Stopping -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.height(56.dp),
                    ) {
                        CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.5.dp)
                        Text(
                            text = stringResource(
                                if (state is LiveRecordingState.Preparing) {
                                    R.string.recording_preparing
                                } else {
                                    R.string.recording_stopping
                                },
                            ),
                            style = MaterialTheme.typography.titleMedium,
                        )
                    }
                }
                recording -> {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        FilledTonalButton(
                            onClick = { if (isPaused) onResume() else onPause() },
                            shape = RoundedCornerShape(24.dp),
                            modifier = Modifier
                                .weight(1f)
                                .height(56.dp),
                            colors = ButtonDefaults.filledTonalButtonColors(
                                containerColor = if (isPaused) {
                                    MaterialTheme.colorScheme.primaryContainer
                                } else {
                                    MaterialTheme.colorScheme.surfaceContainerHighest
                                },
                            ),
                        ) {
                            Icon(
                                imageVector = if (isPaused) Icons.Rounded.PlayArrow else Icons.Rounded.Pause,
                                contentDescription = null,
                                modifier = Modifier.size(22.dp),
                            )
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = stringResource(
                                    if (isPaused) R.string.record_resume_description else R.string.record_pause_description,
                                ),
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }

                        Button(
                            onClick = onStop,
                            shape = RoundedCornerShape(24.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = recordingColor,
                                contentColor = Color.White,
                            ),
                            modifier = Modifier
                                .weight(1.25f)
                                .height(56.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.Stop,
                                contentDescription = null,
                                modifier = Modifier.size(24.dp),
                            )
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = stringResource(R.string.record_stop_description),
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.Bold,
                            )
                        }
                    }
                }
                else -> {
                    val enabled = presentation.isStartAvailable
                    Button(
                        onClick = onStart,
                        enabled = enabled,
                        shape = RoundedCornerShape(28.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = if (bootstrapReady) {
                                MaterialTheme.colorScheme.primary
                            } else {
                                MaterialTheme.colorScheme.secondaryContainer
                            },
                            contentColor = if (bootstrapReady) {
                                MaterialTheme.colorScheme.onPrimary
                            } else {
                                MaterialTheme.colorScheme.onSecondaryContainer
                            },
                        ),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(58.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Mic,
                            contentDescription = null,
                            modifier = Modifier.size(24.dp),
                        )
                        Spacer(Modifier.width(10.dp))
                        Text(
                            text = stringResource(R.string.record_action_description),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold,
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
    configurationMissing: Boolean,
    permissionIssue: MicrophonePermissionDecision?,
    onOpenSetup: () -> Unit,
    onViewCompleted: (String) -> Unit,
    showNotificationNotice: Boolean = false,
    onOpenNotificationSettings: () -> Unit = {},
    shouldShowNotificationRationale: Boolean = false,
) {
    if (state is LiveRecordingState.Completed) {
        Surface(
            shape = RoundedCornerShape(18.dp),
            color = MaterialTheme.colorScheme.primaryContainer,
            contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Icon(
                    imageVector = Icons.Rounded.CheckCircle,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                )
                Text(
                    text = stringResource(
                        if (state.warning == null) R.string.recording_completed else R.string.recording_completed_warning,
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    modifier = Modifier.weight(1f),
                )
                FilledTonalButton(onClick = { onViewCompleted(state.historyId) }) {
                    Text(stringResource(R.string.file_view_result))
                    Spacer(Modifier.width(4.dp))
                    Icon(Icons.AutoMirrored.Rounded.ArrowForward, null, modifier = Modifier.size(16.dp))
                }
            }
        }
    }

    if (configurationMissing || state is LiveRecordingState.NeedsConfiguration || permissionIssue != null) {
        Surface(
            onClick = onOpenSetup,
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.errorContainer,
            contentColor = MaterialTheme.colorScheme.onErrorContainer,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Icon(
                    imageVector = Icons.Rounded.WarningAmber,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                    modifier = Modifier.size(20.dp),
                )
                Text(
                    text = stringResource(
                        when (permissionIssue) {
                            MicrophonePermissionDecision.SHOW_RATIONALE -> R.string.microphone_permission_rationale
                            MicrophonePermissionDecision.OPEN_APP_SETTINGS -> R.string.microphone_permission_settings
                            else -> R.string.recording_needs_configuration
                        },
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    text = stringResource(R.string.action_configure),
                    style = MaterialTheme.typography.labelLarge,
                    fontWeight = FontWeight.Bold,
                )
            }
        }
    }

    if (state is LiveRecordingState.Recording) {
        if (state.streamingStatus is StreamingStatus.AudioOnly) {
            NoticePill(
                text = stringResource(R.string.recording_audio_only),
                isWarning = true,
            )
        }
        when (val inputStatus = state.inputStatus) {
            AudioInputStatus.Active -> Unit
            AudioInputStatus.Silenced -> NoticePill(
                text = stringResource(R.string.microphone_silenced),
                isWarning = true,
            )
            AudioInputStatus.MonitoringUnavailable -> NoticePill(
                text = stringResource(R.string.microphone_monitoring_unavailable),
            )
            is AudioInputStatus.DeviceChanged -> NoticePill(
                text = inputStatus.deviceName?.takeIf(String::isNotBlank)?.let { name ->
                    stringResource(R.string.microphone_device_changed, name)
                } ?: stringResource(R.string.microphone_device_changed_unknown),
            )
        }
    }

    if (permissionIssue == null && showNotificationNotice) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Text(
                    text = stringResource(
                        if (shouldShowNotificationRationale) {
                            R.string.notification_permission_rationale
                        } else {
                            R.string.notification_permission_notice
                        },
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = onOpenNotificationSettings) {
                    Text(stringResource(R.string.action_enable_notifications))
                }
            }
        }
    }
}

@Composable
private fun NoticePill(
    text: String,
    isWarning: Boolean = false,
) {
    Surface(
        shape = RoundedCornerShape(14.dp),
        color = if (isWarning) {
            MaterialTheme.colorScheme.errorContainer
        } else {
            MaterialTheme.colorScheme.surfaceContainerHigh
        },
        contentColor = if (isWarning) {
            MaterialTheme.colorScheme.onErrorContainer
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        },
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(
                imageVector = if (isWarning) Icons.Rounded.WarningAmber else Icons.Rounded.CheckCircle,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
            Text(text = text, style = MaterialTheme.typography.bodySmall)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LiveEngineSelectorSheet(
    recognitionSettings: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
    onSelectModel: (AsrModelSelection?) -> Unit,
    onOpenRecognitionSettings: () -> Unit,
    onDismiss: () -> Unit,
) {
    val localStreamingModels = recognitionSettings.installedModels.filter { it.supports(AsrMode.STREAMING) }
    val current = recognitionSettings.liveSelection

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text(
                text = stringResource(R.string.recognition_live_model),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
            )

            localStreamingModels.forEach { model ->
                val selection = AsrModelSelection.Local(model.id)
                val selected = current == selection
                EngineOptionRow(
                    title = model.displayName,
                    subtitle = model.config.modelType,
                    selected = selected,
                    onClick = { onSelectModel(selection) },
                )
            }

            val onlineSelection = AsrModelSelection.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO)
            val onlineConfigured = OnlineAsrProvider.VOLCENGINE_DOUBAO in configuredProviders
            EngineOptionRow(
                title = stringResource(R.string.batch_provider_volcengine_doubao),
                subtitle = stringResource(
                    if (onlineConfigured) R.string.credential_configured else R.string.credential_not_configured,
                ),
                selected = current == onlineSelection,
                onClick = {
                    if (onlineConfigured) {
                        onSelectModel(onlineSelection)
                    } else {
                        onOpenRecognitionSettings()
                    }
                },
            )

            Spacer(Modifier.height(4.dp))
            OutlinedButton(
                onClick = onOpenRecognitionSettings,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(16.dp),
            ) {
                Icon(Icons.Rounded.Settings, null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.action_configure))
            }
        }
    }
}

@Composable
private fun EngineOptionRow(
    title: String,
    subtitle: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(18.dp))
            .clickable(role = Role.RadioButton, onClick = onClick),
        shape = RoundedCornerShape(18.dp),
        color = if (selected) {
            MaterialTheme.colorScheme.primaryContainer
        } else {
            MaterialTheme.colorScheme.surfaceContainerHigh
        },
        contentColor = if (selected) {
            MaterialTheme.colorScheme.onPrimaryContainer
        } else {
            MaterialTheme.colorScheme.onSurface
        },
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (selected) {
                        MaterialTheme.colorScheme.onPrimaryContainer.copy(alpha = 0.8f)
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                )
            }
            if (selected) {
                Icon(Icons.Rounded.Check, contentDescription = null)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RecordingSetupSheet(
    bootstrapState: SonaBootstrapUiState,
    configurationMissing: Boolean,
    liveSelection: AsrModelSelection?,
    permissionIssue: MicrophonePermissionDecision?,
    onRetryBootstrap: () -> Unit,
    onRetryPermission: () -> Unit,
    onOpenAppSettings: () -> Unit,
    onConfigureCredential: () -> Unit,
    onConfigureRecognition: () -> Unit,
    onSelectEngine: () -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(
                text = stringResource(R.string.record_setup_sheet_title),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
            )
            Text(
                text = stringResource(
                    when {
                        permissionIssue == MicrophonePermissionDecision.SHOW_RATIONALE ->
                            R.string.microphone_permission_rationale
                        permissionIssue == MicrophonePermissionDecision.OPEN_APP_SETTINGS ->
                            R.string.microphone_permission_settings
                        configurationMissing -> R.string.record_setup_sheet_body
                        bootstrapState is SonaBootstrapUiState.Error -> R.string.status_error
                        else -> R.string.status_loading
                    },
                ),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            when {
                permissionIssue == MicrophonePermissionDecision.SHOW_RATIONALE -> {
                    Button(
                        onClick = onRetryPermission,
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(18.dp),
                    ) {
                        Text(stringResource(R.string.action_grant_permission))
                    }
                }
                permissionIssue == MicrophonePermissionDecision.OPEN_APP_SETTINGS -> {
                    Button(
                        onClick = onOpenAppSettings,
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(18.dp),
                    ) {
                        Text(stringResource(R.string.action_open_app_settings))
                    }
                }
                configurationMissing -> {
                    Button(
                        onClick = if (liveSelection is AsrModelSelection.Online) {
                            onConfigureCredential
                        } else {
                            onConfigureRecognition
                        },
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(18.dp),
                    ) {
                        Text(stringResource(R.string.action_configure))
                    }
                    FilledTonalButton(
                        onClick = onSelectEngine,
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(18.dp),
                    ) {
                        Text(stringResource(R.string.recognition_live_model))
                    }
                }
                bootstrapState is SonaBootstrapUiState.Error -> {
                    Button(
                        onClick = onRetryBootstrap,
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(18.dp),
                    ) {
                        Text(stringResource(R.string.action_retry))
                    }
                }
            }
        }
    }
}

@Composable
private fun BootstrapStatus(
    bootstrapState: SonaBootstrapUiState,
    onRetryBootstrap: () -> Unit,
) {
    if (!shouldShowBootstrapStatus(bootstrapState)) return

    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.engine_label) + ": ",
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.width(4.dp))
            when (bootstrapState) {
                SonaBootstrapUiState.Loading -> {
                    CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = stringResource(R.string.status_loading),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                is SonaBootstrapUiState.Ready -> Unit
                is SonaBootstrapUiState.Error -> {
                    Text(
                        text = stringResource(R.string.status_error),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Spacer(Modifier.weight(1f))
                    FilledTonalButton(
                        onClick = onRetryBootstrap,
                        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 2.dp),
                        modifier = Modifier.height(30.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.action_retry),
                            style = MaterialTheme.typography.labelMedium,
                        )
                    }
                }
            }
        }
    }
}

internal fun shouldShowBootstrapStatus(bootstrapState: SonaBootstrapUiState): Boolean =
    bootstrapState !is SonaBootstrapUiState.Ready

@StringRes
private fun RecordingStatusCategory.labelRes(): Int = when (this) {
    RecordingStatusCategory.IDLE -> R.string.recording_idle
    RecordingStatusCategory.NEEDS_CONFIGURATION -> R.string.recording_needs_configuration
    RecordingStatusCategory.PREPARING -> R.string.recording_preparing
    RecordingStatusCategory.RECORDING -> R.string.recording_active
    RecordingStatusCategory.PAUSED -> R.string.recording_paused
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
