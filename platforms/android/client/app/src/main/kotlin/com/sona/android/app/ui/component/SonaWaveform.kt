package com.sona.android.app.ui.component

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Material 3 Expressive audio waveform visualizer rendering a rolling history
 * of normalized amplitude samples (`0f..1f`).
 */
@Composable
fun SonaWaveform(
    amplitudes: List<Float>,
    active: Boolean,
    modifier: Modifier = Modifier,
    height: Dp = 64.dp,
    activeColor: Color = MaterialTheme.colorScheme.primary,
    inactiveColor: Color = MaterialTheme.colorScheme.outlineVariant,
    barCount: Int = 36,
) {
    val activeAlpha by animateFloatAsState(
        targetValue = if (active) 1f else 0.45f,
        animationSpec = spring(dampingRatio = 0.85f, stiffness = 400f),
        label = "waveformAlpha",
    )

    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(height),
    ) {
        val count = barCount.coerceAtLeast(1)
        val totalWidth = size.width
        val slotWidth = totalWidth / count
        val barWidth = (slotWidth * 0.56f).coerceIn(3.dp.toPx(), 7.dp.toPx())
        val cornerRadius = CornerRadius(barWidth / 2f, barWidth / 2f)
        val minBarHeight = 6.dp.toPx()
        val maxBarHeight = size.height * 0.92f
        val centerY = size.height / 2f

        val paddedStart = (count - amplitudes.size).coerceAtLeast(0)
        val visibleOffset = (amplitudes.size - count).coerceAtLeast(0)

        for (index in 0 until count) {
            val rawLevel = if (index < paddedStart) {
                0f
            } else {
                amplitudes[visibleOffset + (index - paddedStart)].coerceIn(0f, 1f)
            }
            val effectiveLevel = if (active) rawLevel else rawLevel * 0.25f
            val barHeight = (minBarHeight + effectiveLevel * (maxBarHeight - minBarHeight))
                .coerceIn(minBarHeight, maxBarHeight)

            // Fade outer bars slightly so the waveform tapers cleanly at the edges
            val centerDistance = kotlin.math.abs(index - (count - 1) / 2f) / ((count - 1) / 2f).coerceAtLeast(1f)
            val edgeAlpha = (1f - centerDistance * 0.35f).coerceIn(0.35f, 1f)
            val color = if (effectiveLevel > 0.04f && active) {
                activeColor.copy(alpha = activeAlpha * edgeAlpha)
            } else {
                inactiveColor.copy(alpha = activeAlpha * edgeAlpha * 0.85f)
            }

            val left = index * slotWidth + (slotWidth - barWidth) / 2f
            drawRoundRect(
                color = color,
                topLeft = Offset(left, centerY - barHeight / 2f),
                size = Size(barWidth, barHeight),
                cornerRadius = cornerRadius,
            )
        }
    }
}
