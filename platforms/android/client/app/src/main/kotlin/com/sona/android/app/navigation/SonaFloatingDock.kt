package com.sona.android.app.navigation

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.sona.android.app.R
import com.sona.android.app.feature.recording.formatRecordingTimer
import com.sona.android.app.ui.theme.LocalSonaRecordingColor
import com.sona.android.application.recording.LiveRecordingState

/**
 * Spring specification for fluid navigation and state transitions.
 */
internal val SonaFluidSpringSpec = spring<Float>(
    dampingRatio = 0.78f,
    stiffness = 420f,
)

/**
 * Layout and styling defaults for the Material 3 Expressive floating capture & navigation dock.
 */
object SonaFloatingDockDefaults {
    val ContainerShape = RoundedCornerShape(32.dp)
    val PillShape = RoundedCornerShape(22.dp)
    val CaptureShape = RoundedCornerShape(28.dp)
    val Elevation = 6.dp
    val HorizontalMargin = 16.dp
    val BottomMargin = 12.dp
    val MaxWidth = 480.dp
    val DockHeight = 62.dp

    fun shouldShow(route: String?, sectionArg: String? = null): Boolean =
        SonaNavMotion.isRootTab(route, sectionArg)
}

/**
 * Material 3 Expressive floating dock pairing the 2-destination navigation pill
 * (`Library` + `Settings`) with a global quick-capture capsule (tap to record immediately,
 * long-press or tap the adjacent file-import button to transcribe an audio file).
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SonaFloatingDock(
    currentRoute: String?,
    onDestinationSelected: (SonaDestination) -> Unit,
    onRecordClick: () -> Unit,
    onImportAudioClick: () -> Unit,
    modifier: Modifier = Modifier,
    recordingState: LiveRecordingState = LiveRecordingState.Idle,
    currentSectionArg: String? = null,
    destinations: List<SonaDestination> = SonaDestination.entries,
    visible: Boolean = SonaFloatingDockDefaults.shouldShow(currentRoute, currentSectionArg),
) {
    val isActiveRecording = recordingState is LiveRecordingState.Preparing ||
        recordingState is LiveRecordingState.Recording ||
        recordingState is LiveRecordingState.Stopping

    AnimatedVisibility(
        visible = visible,
        enter = slideInVertically(
            initialOffsetY = { it },
            animationSpec = spring(
                dampingRatio = Spring.DampingRatioLowBouncy,
                stiffness = Spring.StiffnessMediumLow,
            ),
        ) + fadeIn(
            animationSpec = spring(
                dampingRatio = Spring.DampingRatioNoBouncy,
                stiffness = Spring.StiffnessMedium,
            ),
        ),
        exit = slideOutVertically(
            targetOffsetY = { it },
            animationSpec = spring(
                dampingRatio = Spring.DampingRatioNoBouncy,
                stiffness = Spring.StiffnessMedium,
            ),
        ) + fadeOut(
            animationSpec = spring(
                dampingRatio = Spring.DampingRatioNoBouncy,
                stiffness = Spring.StiffnessMedium,
            ),
        ),
        modifier = modifier,
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .windowInsetsPadding(WindowInsets.navigationBars)
                .padding(
                    horizontal = SonaFloatingDockDefaults.HorizontalMargin,
                    vertical = SonaFloatingDockDefaults.BottomMargin,
                ),
            contentAlignment = Alignment.BottomCenter,
        ) {
            Row(
                modifier = Modifier
                    .widthIn(max = SonaFloatingDockDefaults.MaxWidth)
                    .fillMaxWidth()
                    .height(IntrinsicSize.Min)
                    .defaultMinSize(minHeight = SonaFloatingDockDefaults.DockHeight),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Navigation Pill (Library + Settings)
                Surface(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxHeight()
                        .defaultMinSize(minHeight = SonaFloatingDockDefaults.DockHeight)
                        .shadow(
                            elevation = SonaFloatingDockDefaults.Elevation,
                            shape = SonaFloatingDockDefaults.ContainerShape,
                            clip = false,
                        ),
                    shape = SonaFloatingDockDefaults.ContainerShape,
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    tonalElevation = 2.dp,
                    shadowElevation = 0.dp,
                    border = BorderStroke(
                        width = 0.8.dp,
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.45f),
                    ),
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 6.dp, vertical = 6.dp),
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        destinations.forEach { destination ->
                            val isSelected = destination.matches(currentRoute)
                            SonaFloatingDockItem(
                                destination = destination,
                                isSelected = isSelected,
                                onClick = { onDestinationSelected(destination) },
                                modifier = Modifier
                                    .weight(1f)
                                    .fillMaxHeight(),
                            )
                        }
                    }
                }

                // File Import Quick Button (shown when not actively recording)
                AnimatedVisibility(visible = !isActiveRecording) {
                    val importDesc = stringResource(R.string.file_select_audio)
                    Surface(
                        modifier = Modifier
                            .fillMaxHeight()
                            .defaultMinSize(
                                minWidth = SonaFloatingDockDefaults.DockHeight,
                                minHeight = SonaFloatingDockDefaults.DockHeight,
                            )
                            .shadow(
                                elevation = SonaFloatingDockDefaults.Elevation,
                                shape = CircleShape,
                                clip = false,
                            )
                            .clip(CircleShape)
                            .clickable(
                                role = Role.Button,
                                onClick = onImportAudioClick,
                            )
                            .semantics { contentDescription = importDesc },
                        shape = CircleShape,
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                        border = BorderStroke(
                            width = 0.8.dp,
                            color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.35f),
                        ),
                    ) {
                        Box(
                            modifier = Modifier.padding(horizontal = 18.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.AudioFile,
                                contentDescription = null,
                                modifier = Modifier.size(22.dp),
                            )
                        }
                    }
                }

                // Expressive Record Capsule
                RecordCaptureCapsule(
                    recordingState = recordingState,
                    onClick = onRecordClick,
                    onLongClick = onImportAudioClick,
                    modifier = Modifier.fillMaxHeight(),
                )
            }
        }
    }
}

@Composable
private fun SonaFloatingDockItem(
    destination: SonaDestination,
    isSelected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val label = stringResource(destination.labelRes)

    val containerColor by animateColorAsState(
        targetValue = if (isSelected) {
            MaterialTheme.colorScheme.primaryContainer
        } else {
            Color.Transparent
        },
        animationSpec = spring(dampingRatio = 0.82f, stiffness = 460f),
        label = "dockItemContainerColor",
    )

    val contentColor by animateColorAsState(
        targetValue = if (isSelected) {
            MaterialTheme.colorScheme.onPrimaryContainer
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        },
        animationSpec = spring(dampingRatio = 0.82f, stiffness = 460f),
        label = "dockItemContentColor",
    )

    val pillScale by animateFloatAsState(
        targetValue = if (isSelected) 1f else 0.94f,
        animationSpec = SonaFluidSpringSpec,
        label = "dockItemPillScale",
    )

    Box(
        modifier = modifier
            .clip(SonaFloatingDockDefaults.PillShape)
            .background(
                color = containerColor,
                shape = SonaFloatingDockDefaults.PillShape,
            )
            .clickable(
                interactionSource = interactionSource,
                indication = ripple(bounded = true),
                role = Role.Tab,
                onClick = onClick,
            )
            .semantics { selected = isSelected }
            .padding(horizontal = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center,
            modifier = Modifier
                .graphicsLayer {
                    scaleX = pillScale
                    scaleY = pillScale
                }
                .alpha(if (isSelected) 1f else 0.86f),
        ) {
            Icon(
                imageVector = destination.icon,
                contentDescription = null,
                tint = contentColor,
                modifier = Modifier.size(20.dp),
            )
            Spacer(modifier = Modifier.width(6.dp))
            Text(
                text = label,
                color = contentColor,
                fontSize = 13.sp,
                fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun RecordCaptureCapsule(
    recordingState: LiveRecordingState,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val recordingColor = LocalSonaRecordingColor.current
    val isActive = recordingState is LiveRecordingState.Preparing ||
        recordingState is LiveRecordingState.Recording ||
        recordingState is LiveRecordingState.Stopping
    val isPaused = (recordingState as? LiveRecordingState.Recording)?.isPaused == true
    val elapsedMillis = (recordingState as? LiveRecordingState.Recording)?.elapsedMillis ?: 0L

    val containerColor by animateColorAsState(
        targetValue = when {
            isActive && !isPaused -> recordingColor
            isActive && isPaused -> MaterialTheme.colorScheme.tertiaryContainer
            else -> MaterialTheme.colorScheme.primary
        },
        animationSpec = spring(dampingRatio = 0.85f, stiffness = 420f),
        label = "captureCapsuleBg",
    )
    val contentColor by animateColorAsState(
        targetValue = when {
            isActive && !isPaused -> Color.White
            isActive && isPaused -> MaterialTheme.colorScheme.onTertiaryContainer
            else -> MaterialTheme.colorScheme.onPrimary
        },
        animationSpec = spring(dampingRatio = 0.85f, stiffness = 420f),
        label = "captureCapsuleFg",
    )

    val recordActionLabel = stringResource(R.string.action_record)
    val capsuleDescription = if (isActive) {
        stringResource(R.string.recording_active)
    } else {
        stringResource(R.string.record_action_description)
    }

    Surface(
        modifier = modifier
            .defaultMinSize(minHeight = SonaFloatingDockDefaults.DockHeight)
            .shadow(
                elevation = SonaFloatingDockDefaults.Elevation,
                shape = SonaFloatingDockDefaults.CaptureShape,
                clip = false,
            )
            .clip(SonaFloatingDockDefaults.CaptureShape)
            .combinedClickable(
                role = Role.Button,
                onClick = onClick,
                onLongClick = onLongClick,
            )
            .semantics { contentDescription = capsuleDescription },
        shape = SonaFloatingDockDefaults.CaptureShape,
        color = containerColor,
        contentColor = contentColor,
    ) {
        AnimatedContent(
            targetState = isActive,
            transitionSpec = { fadeIn() togetherWith fadeOut() },
            label = "captureCapsuleContent",
        ) { active ->
            Row(
                modifier = Modifier
                    .defaultMinSize(minHeight = SonaFloatingDockDefaults.DockHeight)
                    .padding(horizontal = 20.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (active) {
                    val pulseAlpha = if (!isPaused) {
                        val transition = rememberInfiniteTransition(label = "dockPulse")
                        transition.animateFloat(
                            initialValue = 1f,
                            targetValue = 0.3f,
                            animationSpec = infiniteRepeatable(
                                animation = tween(700),
                                repeatMode = RepeatMode.Reverse,
                            ),
                            label = "dockPulseAlpha",
                        ).value
                    } else {
                        0.6f
                    }
                    Box(
                        modifier = Modifier
                            .size(10.dp)
                            .alpha(pulseAlpha)
                            .background(contentColor, CircleShape),
                    )
                    Text(
                        text = formatRecordingTimer(elapsedMillis),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                    )
                } else {
                    Icon(
                        imageVector = Icons.Rounded.Mic,
                        contentDescription = null,
                        modifier = Modifier.size(22.dp),
                    )
                    Text(
                        text = recordActionLabel,
                        style = MaterialTheme.typography.labelLarge,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}
