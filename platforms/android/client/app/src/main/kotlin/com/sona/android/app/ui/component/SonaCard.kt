package com.sona.android.app.ui.component

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Standard corner radius and spacing tokens for MD3 & One UI Inset-Grouped cards.
 */
object SonaCardDefaults {
    val CardShape: Shape = RoundedCornerShape(24.dp)
    val SubGroupShape: Shape = RoundedCornerShape(18.dp)
    val Elevation: Dp = 2.dp
    val DividerIndent: Dp = 56.dp
}

/**
 * MD3 Inset-Grouped Card container.
 *
 * Distinctive rounded container (`24.dp`) providing structured section grouping
 * with high-contrast surfaces (`colorScheme.surfaceContainer`).
 */
@Composable
fun InsetGroupedCard(
    modifier: Modifier = Modifier,
    shape: Shape = SonaCardDefaults.CardShape,
    containerColor: Color = MaterialTheme.colorScheme.surfaceContainer,
    elevation: Dp = SonaCardDefaults.Elevation,
    border: BorderStroke? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = shape,
        color = containerColor,
        tonalElevation = elevation,
        shadowElevation = 0.dp,
        border = border,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp),
            content = content,
        )
    }
}

/**
 * Elevated interactive card with spring tactile press scaling (`0.985f`) and MD3 ripple.
 */
@Composable
fun SonaElevatedCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    enabled: Boolean = true,
    shape: Shape = SonaCardDefaults.CardShape,
    containerColor: Color = MaterialTheme.colorScheme.surfaceContainer,
    elevation: Dp = SonaCardDefaults.Elevation,
    border: BorderStroke? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()

    val scale by animateFloatAsState(
        targetValue = if (isPressed && enabled && onClick != null) 0.985f else 1.0f,
        animationSpec = spring(
            dampingRatio = 0.75f,
            stiffness = 400f,
        ),
        label = "elevatedCardPressScale",
    )

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .then(
                if (onClick != null) {
                    Modifier
                        .clip(shape)
                        .clickable(
                            interactionSource = interactionSource,
                            indication = ripple(bounded = true),
                            enabled = enabled,
                            role = Role.Button,
                            onClick = onClick,
                        )
                } else {
                    Modifier
                },
            ),
        shape = shape,
        color = containerColor,
        tonalElevation = elevation,
        shadowElevation = 0.dp,
        border = border,
    ) {
        Column(
            modifier = Modifier.fillMaxWidth(),
            content = content,
        )
    }
}

/**
 * High-contrast two-line list item row adhering to MD3 & One UI structural standards.
 *
 * - Headline: SemiBold titleMedium with high-contrast `onSurface`.
 * - Supporting text: Soft bodyMedium with `onSurfaceVariant`.
 * - Optional leading icon/avatar and trailing affordance (Chevron or Switch).
 */
@Composable
fun TwoLineItemRow(
    headline: String,
    modifier: Modifier = Modifier,
    supportingText: String? = null,
    leadingContent: (@Composable () -> Unit)? = null,
    trailingContent: (@Composable () -> Unit)? = null,
    showDefaultTrailingChevron: Boolean = true,
    onClick: (() -> Unit)? = null,
    enabled: Boolean = true,
) {
    val interactionSource = remember { MutableInteractionSource() }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .defaultMinSize(minHeight = 58.dp)
            .then(
                if (onClick != null) {
                    Modifier.clickable(
                        interactionSource = interactionSource,
                        indication = ripple(bounded = true),
                        enabled = enabled,
                        role = Role.Button,
                        onClick = onClick,
                    )
                } else {
                    Modifier
                },
            )
            .padding(horizontal = 18.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        if (leadingContent != null) {
            Box(
                modifier = Modifier.padding(end = 16.dp),
                contentAlignment = Alignment.Center,
            ) {
                leadingContent()
            }
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .padding(vertical = 2.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            Text(
                text = headline,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = if (enabled) {
                    MaterialTheme.colorScheme.onSurface
                } else {
                    MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                },
            )
            if (!supportingText.isNullOrBlank()) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = supportingText,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (enabled) {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.38f)
                    },
                )
            }
        }

        if (trailingContent != null) {
            Box(
                modifier = Modifier.padding(start = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                trailingContent()
            }
        } else if (showDefaultTrailingChevron && onClick != null) {
            Icon(
                imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                modifier = Modifier.padding(start = 12.dp),
            )
        }
    }
}

/**
 * Inset divider for two-line grouped items, matching leading content alignment.
 */
@Composable
fun TwoLineItemDivider(
    modifier: Modifier = Modifier,
    startIndent: Dp = SonaCardDefaults.DividerIndent,
    color: Color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f),
) {
    HorizontalDivider(
        modifier = modifier.padding(start = startIndent),
        thickness = 0.8.dp,
        color = color,
    )
}
