package com.sona.android.app.navigation

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.EnterExitState
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.animateDp
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MotionScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.dp
import androidx.navigation.NavBackStackEntry

internal enum class NavMotionDirection {
    FORWARD,
    BACKWARD,
    NONE,
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
internal object SonaNavMotion {
    const val ENTER_OFFSET_FRACTION = 0.22f
    const val EXIT_OFFSET_FRACTION = 0.08f
    const val PREDICTIVE_BACK_EXIT_SCALE = 0.90f
    const val PREDICTIVE_BACK_UNDERLYING_SCALE = 0.94f
    const val PREDICTIVE_BACK_CORNER_RADIUS_DP = 28f
    const val PREDICTIVE_BACK_ELEVATION_DP = 8f
    const val PARENT_SCRIM_ALPHA = 0.22f
    const val PREDICTIVE_FADE_DURATION_MS = 420

    /** Matches `androidx.activity.BackEventCompat.EDGE_RIGHT`. */
    const val SWIPE_EDGE_RIGHT = 1

    private val ROOT_TAB_ORDER = listOf(
        SonaDestination.LIBRARY.route,
        SonaDestination.SETTINGS.route,
    )

    private val PredictiveFadeEasing = CubicBezierEasing(0.75f, 0.0f, 1.0f, 1.0f)

    /**
     * Static expressive motion scheme matching [com.sona.android.app.ui.theme.SonaTheme] for
     * non-composable navigation transition lambdas where `MaterialTheme.motionScheme` is unavailable.
     */
    val expressiveMotionScheme: MotionScheme = MotionScheme.expressive()

    fun <T> spatialSpringSpec(): FiniteAnimationSpec<T> =
        expressiveMotionScheme.defaultSpatialSpec()

    fun <T> fastSpatialSpringSpec(): FiniteAnimationSpec<T> =
        expressiveMotionScheme.fastSpatialSpec()

    fun <T> effectsSpringSpec(): FiniteAnimationSpec<T> =
        expressiveMotionScheme.defaultEffectsSpec()

    fun <T> fastEffectsSpringSpec(): FiniteAnimationSpec<T> =
        expressiveMotionScheme.fastEffectsSpec()

    fun boundsSpringSpec(): FiniteAnimationSpec<IntRect> =
        expressiveMotionScheme.defaultSpatialSpec()

    /**
     * Keeps the foreground card opaque through the initial predictive back drag so its rounded
     * corners and elevation remain legible, fading out smoothly during the second half.
     */
    fun predictivePopFadeOutSpec(): FiniteAnimationSpec<Float> = tween(
        durationMillis = PREDICTIVE_FADE_DURATION_MS,
        easing = PredictiveFadeEasing,
    )

    fun isSettingsSubSection(route: String?, sectionArg: String? = null): Boolean {
        if (sectionArg != null && sectionArg.isNotBlank() && sectionArg != "{section}") {
            return true
        }
        if (route == null) return false
        if (!route.startsWith("settings?")) return false
        val sectionValue = route.substringAfter("section=", "").substringBefore('&')
        return sectionValue.isNotBlank() && sectionValue != "{section}"
    }

    fun isSubPage(route: String?, sectionArg: String? = null): Boolean {
        if (route == null) return false
        if (isSettingsSubSection(route, sectionArg)) return true
        return route == LIBRARY_DETAIL_ROUTE ||
            route.startsWith("library/")
    }

    fun isRootTab(route: String?, sectionArg: String? = null): Boolean {
        if (route == null) return false
        if (isSubPage(route, sectionArg)) return false
        val baseRoute = route.substringBefore('?').substringBefore('/')
        return ROOT_TAB_ORDER.contains(baseRoute)
    }

    fun isPeerTabTransition(
        initialRoute: String?,
        targetRoute: String?,
        initialSectionArg: String? = null,
        targetSectionArg: String? = null,
    ): Boolean {
        if (initialRoute == targetRoute && initialSectionArg == targetSectionArg) {
            return false
        }
        return isRootTab(initialRoute, initialSectionArg) &&
            isRootTab(targetRoute, targetSectionArg)
    }

    fun rootTabIndex(route: String?, sectionArg: String? = null): Int {
        if (!isRootTab(route, sectionArg)) return -1
        val baseRoute = route?.substringBefore('?')?.substringBefore('/')
        return ROOT_TAB_ORDER.indexOf(baseRoute)
    }

    fun resolvePredictiveBackSign(swipeEdge: Int): Float =
        if (swipeEdge == SWIPE_EDGE_RIGHT) -1f else 1f

    fun resolvePredictiveBackTransformOrigin(swipeEdge: Int): TransformOrigin =
        if (swipeEdge == SWIPE_EDGE_RIGHT) {
            TransformOrigin(0.25f, 0.5f)
        } else {
            TransformOrigin(0.75f, 0.5f)
        }

    fun resolveMotionDirection(
        initialRoute: String?,
        targetRoute: String?,
        initialSectionArg: String? = null,
        targetSectionArg: String? = null,
        isPop: Boolean = false,
    ): NavMotionDirection {
        if (initialRoute == targetRoute && initialSectionArg == targetSectionArg) {
            return NavMotionDirection.NONE
        }

        // Syncing NavHost route when SettingsScreen internal scaffold returns to settings list
        if (
            !isPop &&
            isSettingsSubSection(initialRoute, initialSectionArg) &&
            isRootTab(targetRoute, targetSectionArg) &&
            targetRoute?.substringBefore('?') == SonaDestination.SETTINGS.route
        ) {
            return NavMotionDirection.NONE
        }

        // On any pop (back navigation), content moves BACKWARD (screen exits toward edge)
        if (isPop) {
            val initialIsRoot = isRootTab(initialRoute, initialSectionArg)
            val targetIsRoot = isRootTab(targetRoute, targetSectionArg)
            return if (initialIsRoot && targetIsRoot) {
                val fromIdx = rootTabIndex(initialRoute, initialSectionArg)
                val toIdx = rootTabIndex(targetRoute, targetSectionArg)
                if (fromIdx >= 0 && toIdx >= 0) {
                    if (toIdx > fromIdx) NavMotionDirection.FORWARD else NavMotionDirection.BACKWARD
                } else {
                    NavMotionDirection.BACKWARD
                }
            } else {
                NavMotionDirection.BACKWARD
            }
        }

        val initialIsSub = isSubPage(initialRoute, initialSectionArg)
        val targetIsSub = isSubPage(targetRoute, targetSectionArg)
        val initialIsRoot = isRootTab(initialRoute, initialSectionArg)
        val targetIsRoot = isRootTab(targetRoute, targetSectionArg)

        return when {
            targetIsSub -> NavMotionDirection.FORWARD
            initialIsRoot && targetIsRoot -> {
                val fromIdx = rootTabIndex(initialRoute, initialSectionArg)
                val toIdx = rootTabIndex(targetRoute, targetSectionArg)
                if (fromIdx >= 0 && toIdx >= 0) {
                    if (toIdx > fromIdx) NavMotionDirection.FORWARD else NavMotionDirection.BACKWARD
                } else {
                    NavMotionDirection.FORWARD
                }
            }
            initialIsSub && targetIsRoot -> NavMotionDirection.BACKWARD
            else -> NavMotionDirection.FORWARD
        }
    }
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.resolveMotionDirection(isPop: Boolean): NavMotionDirection {
    val initialRoute = initialState.destination.route
    val targetRoute = targetState.destination.route
    val initialSection = initialState.arguments?.getString(SETTINGS_SECTION_ARGUMENT)
    val targetSection = targetState.arguments?.getString(SETTINGS_SECTION_ARGUMENT)
    return SonaNavMotion.resolveMotionDirection(
        initialRoute = initialRoute,
        targetRoute = targetRoute,
        initialSectionArg = initialSection,
        targetSectionArg = targetSection,
        isPop = isPop,
    )
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.isPeerTabTransition(): Boolean {
    val initialRoute = initialState.destination.route
    val targetRoute = targetState.destination.route
    val initialSection = initialState.arguments?.getString(SETTINGS_SECTION_ARGUMENT)
    val targetSection = targetState.arguments?.getString(SETTINGS_SECTION_ARGUMENT)
    return SonaNavMotion.isPeerTabTransition(
        initialRoute = initialRoute,
        targetRoute = targetRoute,
        initialSectionArg = initialSection,
        targetSectionArg = targetSection,
    )
}

/**
 * Shared Axis X forward enter for sub-pages (M3 Expressive spatial + effects springs).
 */
internal fun sonaSubPageEnterFromRight(): EnterTransition =
    slideInHorizontally(
        initialOffsetX = { fullWidth -> (fullWidth * SonaNavMotion.ENTER_OFFSET_FRACTION).toInt() },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + fadeIn(
        animationSpec = SonaNavMotion.effectsSpringSpec(),
    )

/**
 * Shared Axis X pop enter for underlying parent pages (gentle recession recovery + scale).
 */
internal fun sonaSubPageEnterFromLeft(
    offsetFraction: Float = -SonaNavMotion.EXIT_OFFSET_FRACTION,
): EnterTransition =
    slideInHorizontally(
        initialOffsetX = { fullWidth -> (fullWidth * offsetFraction).toInt() },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + scaleIn(
        initialScale = SonaNavMotion.PREDICTIVE_BACK_UNDERLYING_SCALE,
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + fadeIn(
        animationSpec = SonaNavMotion.effectsSpringSpec(),
    )

/**
 * Shared Axis X forward exit for underlying parent pages (recedes slightly with subtle scale).
 */
internal fun sonaSubPageExitToLeft(): ExitTransition =
    slideOutHorizontally(
        targetOffsetX = { fullWidth -> -(fullWidth * SonaNavMotion.EXIT_OFFSET_FRACTION).toInt() },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + scaleOut(
        targetScale = SonaNavMotion.PREDICTIVE_BACK_UNDERLYING_SCALE,
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + fadeOut(
        animationSpec = SonaNavMotion.fastEffectsSpringSpec(),
    )

/**
 * Shared Axis X + Predictive Back style exit for sub-pages.
 */
internal fun sonaSubPageExitToRight(
    directionSign: Float = 1f,
    transformOrigin: TransformOrigin = TransformOrigin(0.75f, 0.5f),
    fadeSpec: FiniteAnimationSpec<Float> = SonaNavMotion.effectsSpringSpec(),
): ExitTransition =
    slideOutHorizontally(
        targetOffsetX = { fullWidth ->
            (fullWidth * SonaNavMotion.ENTER_OFFSET_FRACTION * directionSign).toInt()
        },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + scaleOut(
        targetScale = SonaNavMotion.PREDICTIVE_BACK_EXIT_SCALE,
        transformOrigin = transformOrigin,
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    ) + fadeOut(
        animationSpec = fadeSpec,
    )

/**
 * Lightweight M3 Expressive FadeThrough for Two-Pane detail section switching.
 */
internal fun sonaTwoPaneSectionTransition(): ContentTransform =
    (
        fadeIn(animationSpec = SonaNavMotion.effectsSpringSpec()) +
            slideInVertically(
                initialOffsetY = { fullHeight -> (fullHeight * 0.03f).toInt() },
                animationSpec = SonaNavMotion.spatialSpringSpec(),
            )
    ) togetherWith fadeOut(animationSpec = SonaNavMotion.fastEffectsSpringSpec())

private fun peerTabEnterTransition(direction: NavMotionDirection): EnterTransition = when (direction) {
    NavMotionDirection.FORWARD -> slideInHorizontally(
        initialOffsetX = { fullWidth -> fullWidth },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    )
    NavMotionDirection.BACKWARD -> slideInHorizontally(
        initialOffsetX = { fullWidth -> -fullWidth },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    )
    NavMotionDirection.NONE -> EnterTransition.None
}

private fun peerTabExitTransition(direction: NavMotionDirection): ExitTransition = when (direction) {
    NavMotionDirection.FORWARD -> slideOutHorizontally(
        targetOffsetX = { fullWidth -> -fullWidth },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    )
    NavMotionDirection.BACKWARD -> slideOutHorizontally(
        targetOffsetX = { fullWidth -> fullWidth },
        animationSpec = SonaNavMotion.spatialSpringSpec(),
    )
    NavMotionDirection.NONE -> ExitTransition.None
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaEnterTransition(): EnterTransition {
    val direction = resolveMotionDirection(isPop = false)
    if (isPeerTabTransition()) {
        return peerTabEnterTransition(direction)
    }
    return when (direction) {
        NavMotionDirection.FORWARD -> sonaSubPageEnterFromRight()
        NavMotionDirection.BACKWARD -> sonaSubPageEnterFromLeft()
        NavMotionDirection.NONE -> EnterTransition.None
    }
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaExitTransition(): ExitTransition {
    val direction = resolveMotionDirection(isPop = false)
    if (isPeerTabTransition()) {
        return peerTabExitTransition(direction)
    }
    return when (direction) {
        NavMotionDirection.FORWARD -> sonaSubPageExitToLeft()
        NavMotionDirection.BACKWARD -> sonaSubPageExitToRight()
        NavMotionDirection.NONE -> ExitTransition.None
    }
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaPopEnterTransition(): EnterTransition {
    val direction = resolveMotionDirection(isPop = true)
    if (isPeerTabTransition()) {
        return peerTabEnterTransition(direction)
    }
    return when (direction) {
        NavMotionDirection.BACKWARD -> sonaSubPageEnterFromLeft()
        NavMotionDirection.FORWARD -> sonaSubPageEnterFromRight()
        NavMotionDirection.NONE -> EnterTransition.None
    }
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaPopExitTransition(): ExitTransition {
    val direction = resolveMotionDirection(isPop = true)
    if (isPeerTabTransition()) {
        return peerTabExitTransition(direction)
    }
    return when (direction) {
        NavMotionDirection.BACKWARD -> sonaSubPageExitToRight()
        NavMotionDirection.FORWARD -> sonaSubPageExitToRight(
            directionSign = -1f,
            transformOrigin = TransformOrigin(0.25f, 0.5f),
        )
        NavMotionDirection.NONE -> ExitTransition.None
    }
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaPredictivePopEnterTransition(
    swipeEdge: Int,
): EnterTransition {
    val direction = resolveMotionDirection(isPop = true)
    if (isPeerTabTransition()) {
        val mirrored = if (swipeEdge == SonaNavMotion.SWIPE_EDGE_RIGHT) {
            when (direction) {
                NavMotionDirection.BACKWARD -> NavMotionDirection.FORWARD
                NavMotionDirection.FORWARD -> NavMotionDirection.BACKWARD
                NavMotionDirection.NONE -> NavMotionDirection.NONE
            }
        } else {
            direction
        }
        return peerTabEnterTransition(mirrored)
    }
    val sign = SonaNavMotion.resolvePredictiveBackSign(swipeEdge)
    return when (direction) {
        NavMotionDirection.BACKWARD,
        NavMotionDirection.FORWARD,
        -> sonaSubPageEnterFromLeft(
            offsetFraction = -SonaNavMotion.EXIT_OFFSET_FRACTION * sign,
        )
        NavMotionDirection.NONE -> EnterTransition.None
    }
}

internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaPredictivePopExitTransition(
    swipeEdge: Int,
): ExitTransition {
    val direction = resolveMotionDirection(isPop = true)
    if (isPeerTabTransition()) {
        val mirrored = if (swipeEdge == SonaNavMotion.SWIPE_EDGE_RIGHT) {
            when (direction) {
                NavMotionDirection.BACKWARD -> NavMotionDirection.FORWARD
                NavMotionDirection.FORWARD -> NavMotionDirection.BACKWARD
                NavMotionDirection.NONE -> NavMotionDirection.NONE
            }
        } else {
            direction
        }
        return peerTabExitTransition(mirrored)
    }
    val sign = SonaNavMotion.resolvePredictiveBackSign(swipeEdge)
    val origin = SonaNavMotion.resolvePredictiveBackTransformOrigin(swipeEdge)
    return when (direction) {
        NavMotionDirection.BACKWARD,
        NavMotionDirection.FORWARD,
        -> sonaSubPageExitToRight(
            directionSign = sign,
            transformOrigin = origin,
            fadeSpec = SonaNavMotion.predictivePopFadeOutSpec(),
        )
        NavMotionDirection.NONE -> ExitTransition.None
    }
}

/**
 * Applies rounded-corner clipping and smoothly interpolated elevation shadow to foreground sub-pages
 * during enter, exit, and predictive back gestures while keeping a stable composition tree and
 * reading animated state solely inside `graphicsLayer` (draw phase, zero per-frame recomposition).
 */
@Composable
internal fun SonaSubPageTransitionContainer(
    scope: AnimatedVisibilityScope,
    enabled: Boolean = true,
    content: @Composable () -> Unit,
) {
    val cornerRadiusState = scope.transition.animateDp(
        transitionSpec = { SonaNavMotion.spatialSpringSpec() },
        label = "subPageCornerRadius",
    ) { state ->
        if (enabled && state != EnterExitState.Visible) {
            SonaNavMotion.PREDICTIVE_BACK_CORNER_RADIUS_DP.dp
        } else {
            0.dp
        }
    }
    val density = LocalDensity.current
    val maxCornerPx = with(density) { SonaNavMotion.PREDICTIVE_BACK_CORNER_RADIUS_DP.dp.toPx() }
    val elevationPx = with(density) { SonaNavMotion.PREDICTIVE_BACK_ELEVATION_DP.dp.toPx() }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .graphicsLayer {
                if (enabled) {
                    val cornerPx = cornerRadiusState.value.toPx().coerceAtLeast(0f)
                    if (cornerPx > 0.5f) {
                        val progress = if (maxCornerPx > 0f) {
                            (cornerPx / maxCornerPx).coerceIn(0f, 1f)
                        } else {
                            0f
                        }
                        shape = RoundedCornerShape(cornerPx)
                        clip = true
                        shadowElevation = elevationPx * progress
                    }
                }
            },
    ) {
        content()
    }
}

/**
 * Applies a subtle darkening scrim over underlying parent pages when a sub-page is pushed
 * or scrubbed back via predictive back.
 */
@Composable
internal fun SonaParentPageTransitionContainer(
    scope: AnimatedVisibilityScope,
    enableScrim: Boolean = true,
    content: @Composable () -> Unit,
) {
    val scrimAlpha by scope.transition.animateFloat(
        transitionSpec = { SonaNavMotion.effectsSpringSpec() },
        label = "parentPageScrimAlpha",
    ) { state ->
        if (enableScrim && state != EnterExitState.Visible) {
            SonaNavMotion.PARENT_SCRIM_ALPHA
        } else {
            0f
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .drawWithContent {
                drawContent()
                if (scrimAlpha > 0.001f) {
                    drawRect(color = Color.Black.copy(alpha = scrimAlpha))
                }
            },
    ) {
        content()
    }
}
