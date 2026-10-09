package com.sona.android.app.navigation

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.FastOutLinearInEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.navigation.NavBackStackEntry

/**
 * Standard fluid easing curve: starts fast, decelerates smoothly into place.
 */
internal val SonaMotionEasing = CubicBezierEasing(0.2f, 0.0f, 0.0f, 1.0f)

internal enum class NavMotionDirection {
    FORWARD,
    BACKWARD,
    NONE,
}

internal object SonaNavMotion {
    const val SLIDE_DURATION = 280
    const val FADE_IN_DURATION = 200
    const val FADE_OUT_DURATION = 180
    const val POP_EXIT_FADE_DURATION = 200
    const val ENTER_OFFSET_FRACTION = 0.18f
    const val EXIT_OFFSET_FRACTION = 0.08f

    private val ROOT_TAB_ORDER = listOf(
        SonaDestination.HOME.route,
        SonaDestination.LIBRARY.route,
        SonaDestination.SETTINGS.route,
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
        return route == HOME_LIVE_ROUTE ||
            route == HOME_FILE_ROUTE ||
            route == LIBRARY_DETAIL_ROUTE ||
            route.startsWith("library/")
    }

    fun isRootTab(route: String?, sectionArg: String? = null): Boolean {
        if (route == null) return false
        if (isSubPage(route, sectionArg)) return false
        val baseRoute = route.substringBefore('?').substringBefore('/')
        return ROOT_TAB_ORDER.contains(baseRoute)
    }

    fun rootTabIndex(route: String?, sectionArg: String? = null): Int {
        if (!isRootTab(route, sectionArg)) return -1
        val baseRoute = route?.substringBefore('?')?.substringBefore('/')
        return ROOT_TAB_ORDER.indexOf(baseRoute)
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

        // On any pop (back navigation), content moves BACKWARD (screen slides out to right)
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
            // Forward push to subpage (from root or another subpage)
            targetIsSub -> NavMotionDirection.FORWARD
            // Between Root Tabs: directional horizontal slide based on tab index
            initialIsRoot && targetIsRoot -> {
                val fromIdx = rootTabIndex(initialRoute, initialSectionArg)
                val toIdx = rootTabIndex(targetRoute, targetSectionArg)
                if (fromIdx >= 0 && toIdx >= 0) {
                    if (toIdx > fromIdx) NavMotionDirection.FORWARD else NavMotionDirection.BACKWARD
                } else {
                    NavMotionDirection.FORWARD
                }
            }
            // Returning from subpage to root on a forward navigate call
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

/**
 * Forward enter:
 * - FORWARD: slides in from right (+100% -> 0%)
 * - BACKWARD: slides in from left (-100% -> 0%)
 */
internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaEnterTransition(): EnterTransition {
    return when (resolveMotionDirection(isPop = false)) {
        NavMotionDirection.FORWARD -> {
            slideInHorizontally(
                initialOffsetX = { fullWidth -> (fullWidth * SonaNavMotion.ENTER_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeIn(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.FADE_IN_DURATION,
                    easing = LinearOutSlowInEasing,
                ),
            )
        }
        NavMotionDirection.BACKWARD -> {
            slideInHorizontally(
                initialOffsetX = { fullWidth -> -(fullWidth * SonaNavMotion.ENTER_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeIn(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.FADE_IN_DURATION,
                    easing = LinearOutSlowInEasing,
                ),
            )
        }
        NavMotionDirection.NONE -> EnterTransition.None
    }
}

/**
 * Forward exit:
 * - FORWARD: slides out to left with subtle recession (-8% offset + fade out)
 * - BACKWARD: slides out to right (+8% offset + fade out)
 */
internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaExitTransition(): ExitTransition {
    return when (resolveMotionDirection(isPop = false)) {
        NavMotionDirection.FORWARD -> {
            slideOutHorizontally(
                targetOffsetX = { fullWidth -> -(fullWidth * SonaNavMotion.EXIT_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeOut(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.FADE_OUT_DURATION,
                    easing = FastOutLinearInEasing,
                ),
            )
        }
        NavMotionDirection.BACKWARD -> {
            slideOutHorizontally(
                targetOffsetX = { fullWidth -> (fullWidth * SonaNavMotion.EXIT_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeOut(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.FADE_OUT_DURATION,
                    easing = FastOutLinearInEasing,
                ),
            )
        }
        NavMotionDirection.NONE -> ExitTransition.None
    }
}

/**
 * Pop enter:
 * - BACKWARD: previous screen enters from left subtle recession (-8% -> 0% + fade in)
 * - FORWARD: previous screen enters from right (+8% -> 0% + fade in)
 */
internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaPopEnterTransition(): EnterTransition {
    return when (resolveMotionDirection(isPop = true)) {
        NavMotionDirection.BACKWARD -> {
            slideInHorizontally(
                initialOffsetX = { fullWidth -> -(fullWidth * SonaNavMotion.EXIT_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeIn(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.FADE_IN_DURATION,
                    easing = LinearOutSlowInEasing,
                ),
            )
        }
        NavMotionDirection.FORWARD -> {
            slideInHorizontally(
                initialOffsetX = { fullWidth -> (fullWidth * SonaNavMotion.EXIT_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeIn(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.FADE_IN_DURATION,
                    easing = LinearOutSlowInEasing,
                ),
            )
        }
        NavMotionDirection.NONE -> EnterTransition.None
    }
}

/**
 * Pop exit:
 * - BACKWARD: exiting screen slides out to right with proportional exit (+18% offset + fade out)
 * - FORWARD: exiting screen slides out to left (-18% offset + fade out)
 */
internal fun AnimatedContentTransitionScope<NavBackStackEntry>.sonaPopExitTransition(): ExitTransition {
    return when (resolveMotionDirection(isPop = true)) {
        NavMotionDirection.BACKWARD -> {
            slideOutHorizontally(
                targetOffsetX = { fullWidth -> (fullWidth * SonaNavMotion.ENTER_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeOut(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.POP_EXIT_FADE_DURATION,
                    easing = FastOutLinearInEasing,
                ),
            )
        }
        NavMotionDirection.FORWARD -> {
            slideOutHorizontally(
                targetOffsetX = { fullWidth -> -(fullWidth * SonaNavMotion.ENTER_OFFSET_FRACTION).toInt() },
                animationSpec = tween(
                    durationMillis = SonaNavMotion.SLIDE_DURATION,
                    easing = SonaMotionEasing,
                ),
            ) + fadeOut(
                animationSpec = tween(
                    durationMillis = SonaNavMotion.POP_EXIT_FADE_DURATION,
                    easing = FastOutLinearInEasing,
                ),
            )
        }
        NavMotionDirection.NONE -> ExitTransition.None
    }
}
