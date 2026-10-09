package com.sona.android.app.ui.component

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.TopAppBarScrollBehavior
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import kotlin.math.abs

/**
 * Spring specification for physics-based overscroll bounce.
 */
val SonaOverscrollSpringSpec = spring<Float>(
    dampingRatio = 0.75f,
    stiffness = 300f,
)

/**
 * Pure calculation function for damped overscroll displacement.
 *
 * Applies progressively stronger resistance as displacement approaches [maxDisplacement].
 */
fun calculateDampedOverscroll(
    currentDisplacement: Float,
    delta: Float,
    maxDisplacement: Float,
): Float {
    if (maxDisplacement <= 0f) return 0f
    val currentRatio = (abs(currentDisplacement) / maxDisplacement).coerceIn(0f, 1f)
    val resistance = (1f - currentRatio * 0.75f) * 0.38f
    val updated = currentDisplacement + (delta * resistance)
    return updated.coerceIn(-maxDisplacement, maxDisplacement)
}

/**
 * Attaches physical spring-damped overscroll bounce to scrollable containers.
 *
 * When the user scrolls past the container boundary, a smooth proportional stretch
 * is applied, returning naturally to rest with [SonaOverscrollSpringSpec] on release.
 */
@OptIn(ExperimentalMaterial3Api::class)
fun Modifier.springOverscroll(
    enabled: Boolean = true,
    maxOverscroll: Dp = 80.dp,
    scrollBehavior: TopAppBarScrollBehavior? = null,
): Modifier = composed {
    if (!enabled) return@composed this

    val density = LocalDensity.current
    val maxPx = with(density) { maxOverscroll.toPx() }
    val coroutineScope = rememberCoroutineScope()
    val displacement = remember { Animatable(0f) }

    val connection = remember(maxPx, scrollBehavior) {
        object : NestedScrollConnection {
            override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                val current = displacement.value
                if (current == 0f) return Offset.Zero

                // When pulled down (>0) and dragging back up (<0)
                if (current > 0f && available.y < 0f) {
                    val consumed = available.y.coerceAtLeast(-current)
                    coroutineScope.launch { displacement.snapTo(current + consumed) }
                    return Offset(0f, consumed)
                }
                // When pulled up (<0) and dragging back down (>0)
                if (current < 0f && available.y > 0f) {
                    val consumed = available.y.coerceAtMost(-current)
                    coroutineScope.launch { displacement.snapTo(current + consumed) }
                    return Offset(0f, consumed)
                }
                return Offset.Zero
            }

            override fun onPostScroll(
                consumed: Offset,
                available: Offset,
                source: NestedScrollSource,
            ): Offset {
                if (source != NestedScrollSource.UserInput || available.y == 0f) {
                    return Offset.Zero
                }
                // If scrolling downwards at the top while the top app bar is collapsed/expanding,
                // allow the top app bar to fully expand before initiating downward overscroll stretch.
                val topBarIsCollapsed = scrollBehavior != null && (scrollBehavior.state.collapsedFraction > 0.001f)
                if (available.y > 0f && topBarIsCollapsed) {
                    return Offset.Zero
                }

                val updated = calculateDampedOverscroll(
                    currentDisplacement = displacement.value,
                    delta = available.y,
                    maxDisplacement = maxPx,
                )
                coroutineScope.launch { displacement.snapTo(updated) }
                // Never starve parent NestedScrollConnections (such as TopAppBarScrollBehavior)
                return Offset.Zero
            }
            override suspend fun onPreFling(available: Velocity): Velocity {
                if (displacement.value != 0f) {
                    displacement.animateTo(0f, animationSpec = SonaOverscrollSpringSpec)
                }
                return Velocity.Zero
            }

            override suspend fun onPostFling(consumed: Velocity, available: Velocity): Velocity {
                if (displacement.value != 0f) {
                    displacement.animateTo(0f, animationSpec = SonaOverscrollSpringSpec)
                }
                return Velocity.Zero
            }
        }
    }

    this
        .nestedScroll(connection)
        .graphicsLayer {
            translationY = displacement.value
        }
}
