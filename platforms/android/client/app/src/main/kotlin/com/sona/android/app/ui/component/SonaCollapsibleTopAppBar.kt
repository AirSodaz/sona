package com.sona.android.app.ui.component

import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarColors
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.TopAppBarScrollBehavior
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.sp

/**
 * MD3 colors for collapsible large-title TopAppBars.
 * Seamless background at rest, surfaceContainer tint upon scroll elevation.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun sonaCollapsibleTopAppBarColors(): TopAppBarColors = TopAppBarDefaults.largeTopAppBarColors(
    containerColor = MaterialTheme.colorScheme.background,
    scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer,
    titleContentColor = MaterialTheme.colorScheme.primary,
    navigationIconContentColor = MaterialTheme.colorScheme.onSurface,
    actionIconContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
)

/**
 * One UI / MD3 two-phase collapsible large-title top app bar.
 *
 * - Viewing area (expanded): prominent 34.sp bold primary title for effortless one-handed scanning.
 * - Interaction area (collapsed): smoothly recedes to standard 22.sp title in surfaceContainer.
 * - Driven by standard [TopAppBarScrollBehavior].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SonaCollapsibleTopAppBar(
    title: String,
    modifier: Modifier = Modifier,
    navigationIcon: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    scrollBehavior: TopAppBarScrollBehavior? = null,
    colors: TopAppBarColors = sonaCollapsibleTopAppBarColors(),
    windowInsets: WindowInsets = TopAppBarDefaults.windowInsets,
) {
    val collapsedFraction = scrollBehavior?.state?.collapsedFraction ?: 0f

    LargeTopAppBar(
        title = {
            Text(
                text = title,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                style = if (collapsedFraction > 0.45f) {
                    MaterialTheme.typography.titleLarge.copy(
                        fontSize = 22.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                } else {
                    MaterialTheme.typography.headlineLarge.copy(
                        fontSize = 34.sp,
                        fontWeight = FontWeight.Bold,
                    )
                },
            )
        },
        modifier = modifier,
        navigationIcon = navigationIcon,
        actions = actions,
        scrollBehavior = scrollBehavior,
        colors = colors,
        windowInsets = windowInsets,
    )
}

/**
 * Composable title overload for [SonaCollapsibleTopAppBar].
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SonaCollapsibleTopAppBar(
    title: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    navigationIcon: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    scrollBehavior: TopAppBarScrollBehavior? = null,
    colors: TopAppBarColors = sonaCollapsibleTopAppBarColors(),
    windowInsets: WindowInsets = TopAppBarDefaults.windowInsets,
) {
    LargeTopAppBar(
        title = title,
        modifier = modifier,
        navigationIcon = navigationIcon,
        actions = actions,
        scrollBehavior = scrollBehavior,
        colors = colors,
        windowInsets = windowInsets,
    )
}
