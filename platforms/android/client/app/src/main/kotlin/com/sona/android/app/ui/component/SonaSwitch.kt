package com.sona.android.app.ui.component

import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * Spring specification for morphing state indicators and switch thumbs.
 */
internal val SonaSwitchSpringSpec = spring<Float>(
    dampingRatio = 0.75f,
    stiffness = 380f,
)

/**
 * Material 3 switch with an expressive morphing state indicator in the thumb.
 *
 * - Checked: Scales up check icon (16.dp) with onPrimary contrast.
 * - Unchecked: Shrinks to subtle close icon (10.dp) with onSurfaceVariant tone.
 * - Track & thumb colors strictly aligned with MD3 token standards.
 */
@Composable
fun SonaSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val iconSize by animateDpAsState(
        targetValue = if (checked) 16.dp else 10.dp,
        animationSpec = spring(
            dampingRatio = 0.75f,
            stiffness = 380f,
        ),
        label = "switchIconSize",
    )

    Switch(
        checked = checked,
        onCheckedChange = onCheckedChange,
        modifier = modifier,
        enabled = enabled,
        thumbContent = {
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier.size(SwitchDefaults.IconSize),
            ) {
                Crossfade(
                    targetState = checked,
                    animationSpec = spring(
                        dampingRatio = 0.75f,
                        stiffness = 380f,
                    ),
                    label = "switchThumbMorph",
                ) { isChecked ->
                    if (isChecked) {
                        Icon(
                            imageVector = Icons.Rounded.Check,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onPrimary,
                            modifier = Modifier.size(iconSize),
                        )
                    } else {
                        Icon(
                            imageVector = Icons.Rounded.Close,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(iconSize),
                        )
                    }
                }
            }
        },
        colors = SwitchDefaults.colors(
            checkedThumbColor = MaterialTheme.colorScheme.primary,
            checkedTrackColor = MaterialTheme.colorScheme.primaryContainer,
            checkedBorderColor = MaterialTheme.colorScheme.primary,
            uncheckedThumbColor = MaterialTheme.colorScheme.outline,
            uncheckedTrackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
            uncheckedBorderColor = MaterialTheme.colorScheme.outline,
        ),
    )
}
