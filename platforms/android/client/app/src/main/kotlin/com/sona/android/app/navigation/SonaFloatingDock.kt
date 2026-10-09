package com.sona.android.app.navigation

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Spring specification for fluid navigation and state transitions.
 */
internal val SonaFluidSpringSpec = spring<Float>(
    dampingRatio = 0.78f,
    stiffness = 420f,
)

/**
 * Layout and styling defaults for the floating bottom navigation dock.
 */
object SonaFloatingDockDefaults {
    val ContainerShape = RoundedCornerShape(32.dp)
    val PillShape = RoundedCornerShape(16.dp)
    val Elevation = 8.dp
    val HorizontalMargin = 20.dp
    val BottomMargin = 12.dp
    val MaxWidth = 440.dp
    val DockHeight = 66.dp

    fun shouldShow(route: String?, sectionArg: String? = null): Boolean =
        SonaNavMotion.isRootTab(route, sectionArg)
}

/**
 * Opaque floating bottom navigation dock adhering to MD3 and One UI guidelines.
 *
 * - Container is 100% opaque (solid surfaceContainer).
 * - Floats above navigation bars with 20.dp horizontal margins and max 440.dp width.
 * - Pill indicator smoothly morphs with fluid spring animation.
 * - Automatically animates out on sub-pages and detail workspaces.
 */
@Composable
fun SonaFloatingDock(
    currentRoute: String?,
    onDestinationSelected: (SonaDestination) -> Unit,
    modifier: Modifier = Modifier,
    currentSectionArg: String? = null,
    destinations: List<SonaDestination> = SonaDestination.entries,
    visible: Boolean = SonaFloatingDockDefaults.shouldShow(currentRoute, currentSectionArg),
) {
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
            Surface(
                modifier = Modifier
                    .widthIn(max = SonaFloatingDockDefaults.MaxWidth)
                    .fillMaxWidth()
                    .height(SonaFloatingDockDefaults.DockHeight)
                    .shadow(
                        elevation = SonaFloatingDockDefaults.Elevation,
                        shape = SonaFloatingDockDefaults.ContainerShape,
                        clip = false,
                    ),
                shape = SonaFloatingDockDefaults.ContainerShape,
                color = MaterialTheme.colorScheme.surfaceContainer,
                tonalElevation = 0.dp,
                shadowElevation = 0.dp,
                border = BorderStroke(
                    width = 1.dp,
                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                ),
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    destinations.forEach { destination ->
                        val isSelected = destination.matches(currentRoute)
                        SonaFloatingDockItem(
                            destination = destination,
                            isSelected = isSelected,
                            onClick = { onDestinationSelected(destination) },
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
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

    val iconColor by animateColorAsState(
        targetValue = if (isSelected) {
            MaterialTheme.colorScheme.onPrimaryContainer
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        },
        animationSpec = spring(dampingRatio = 0.8f, stiffness = 500f),
        label = "dockItemIconColor",
    )

    val textColor by animateColorAsState(
        targetValue = if (isSelected) {
            MaterialTheme.colorScheme.onSurface
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        },
        animationSpec = spring(dampingRatio = 0.8f, stiffness = 500f),
        label = "dockItemTextColor",
    )

    val pillWidthFraction by animateFloatAsState(
        targetValue = if (isSelected) 1.0f else 0.0f,
        animationSpec = SonaFluidSpringSpec,
        label = "dockItemPillFraction",
    )

    Column(
        modifier = modifier
            .clip(RoundedCornerShape(20.dp))
            .clickable(
                interactionSource = interactionSource,
                indication = ripple(bounded = true),
                role = Role.Tab,
                onClick = onClick,
            )
            .semantics {
                selected = isSelected
            }
            .padding(vertical = 2.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            modifier = Modifier
                .width(56.dp)
                .height(30.dp),
            contentAlignment = Alignment.Center,
        ) {
            if (pillWidthFraction > 0.01f) {
                Box(
                    modifier = Modifier
                        .width((56 * pillWidthFraction).dp)
                        .height(30.dp)
                        .background(
                            color = MaterialTheme.colorScheme.primaryContainer.copy(
                                alpha = pillWidthFraction.coerceIn(0f, 1f),
                            ),
                            shape = SonaFloatingDockDefaults.PillShape,
                        ),
                )
            }
            Icon(
                imageVector = destination.icon,
                contentDescription = null,
                tint = iconColor,
                modifier = Modifier.size(22.dp),
            )
        }

        Spacer(modifier = Modifier.height(2.dp))

        Text(
            text = label,
            color = textColor,
            fontSize = 11.sp,
            fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
            maxLines = 1,
        )
    }
}
