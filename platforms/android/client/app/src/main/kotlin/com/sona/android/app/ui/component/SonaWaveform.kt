package com.sona.android.app.ui.component

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.ui.theme.LocalSonaRecordingColor
import com.sona.android.application.recording.AudioEnergyBands
import com.sona.android.application.recording.LiveRecordingState
import kotlin.math.PI
import kotlin.math.exp
import kotlin.math.ln
import kotlin.math.pow
import kotlin.math.sin

enum class WaveformVisualMode {
    IDLE,
    PREPARING,
    ACTIVE,
    PAUSED,
}

fun LiveRecordingState.toWaveformVisualMode(): WaveformVisualMode = when (this) {
    LiveRecordingState.Idle,
    LiveRecordingState.NeedsConfiguration,
    is LiveRecordingState.Completed,
    is LiveRecordingState.Failed,
    -> WaveformVisualMode.IDLE

    is LiveRecordingState.Preparing,
    is LiveRecordingState.Stopping,
    -> WaveformVisualMode.PREPARING

    is LiveRecordingState.Recording -> if (isPaused) {
        WaveformVisualMode.PAUSED
    } else {
        WaveformVisualMode.ACTIVE
    }
}

internal fun smoothEnvelopeStep(
    current: Float,
    target: Float,
    dtSeconds: Float,
    attackRate: Float = 18f,
    releaseRate: Float = 5.5f,
): Float {
    val clampedDt = dtSeconds.coerceIn(0f, 0.1f)
    if (clampedDt == 0f) return current
    val rate = if (target >= current) attackRate else releaseRate
    val alpha = 1f - exp(-rate * clampedDt)
    return (current + (target - current) * alpha).coerceIn(0f, 1f)
}

internal fun edgeAttenuationWindow(
    fraction: Float,
    peakCenterFraction: Float = 0.5f,
    exponent: Float = 1.6f,
): Float {
    val u = fraction.coerceIn(0f, 1f)
    if (u <= 0f || u >= 1f) return 0f
    val center = peakCenterFraction.coerceIn(0.2f, 0.8f)
    val warpedU = if (center == 0.5f) {
        u
    } else {
        val gamma = (ln(0.5) / ln(center.toDouble())).toFloat()
        u.pow(gamma)
    }
    return sin(PI * warpedU).toFloat().coerceAtLeast(0f).pow(exponent)
}

internal fun evaluateWaveDisplacement(
    fraction: Float,
    cycles: Float,
    phaseRad: Float,
    amplitudePx: Float,
    peakCenterFraction: Float,
): Float {
    if (amplitudePx <= 0f) return 0f
    val u = fraction.coerceIn(0f, 1f)
    val window = edgeAttenuationWindow(u, peakCenterFraction = peakCenterFraction)
    if (window <= 0f) return 0f
    val primaryWave = sin(u * cycles * (2f * PI.toFloat()) + phaseRad)
    val secondaryHarmonic = sin(u * cycles * 1.85f * (2f * PI.toFloat()) - phaseRad * 0.72f)
    val envelopeShape = 0.58f + 0.42f * (0.68f * primaryWave + 0.32f * secondaryHarmonic)
    return (amplitudePx * window * envelopeShape).coerceAtLeast(0f)
}

private class WaveformPhysicsState {
    var low: Float = 0f
    var mid: Float = 0f
    var high: Float = 0f
    var phaseRad: Float = 0f

    fun step(
        targetBands: AudioEnergyBands,
        mode: WaveformVisualMode,
        dtSeconds: Float,
    ) {
        val targetLow: Float
        val targetMid: Float
        val targetHigh: Float
        val baseAngularSpeed: Float

        when (mode) {
            WaveformVisualMode.ACTIVE -> {
                targetLow = targetBands.low.coerceIn(0f, 1f)
                targetMid = targetBands.mid.coerceIn(0f, 1f)
                targetHigh = targetBands.high.coerceIn(0f, 1f)
                baseAngularSpeed = 2.4f
            }
            WaveformVisualMode.PREPARING -> {
                val pulse = 0.22f + 0.10f * sin(phaseRad * 1.4f)
                targetLow = pulse
                targetMid = pulse * 0.85f
                targetHigh = pulse * 0.65f
                baseAngularSpeed = 2.1f
            }
            WaveformVisualMode.PAUSED -> {
                targetLow = 0f
                targetMid = 0f
                targetHigh = 0f
                baseAngularSpeed = 0.55f
            }
            WaveformVisualMode.IDLE -> {
                targetLow = 0f
                targetMid = 0f
                targetHigh = 0f
                baseAngularSpeed = 1.05f
            }
        }

        low = smoothEnvelopeStep(low, targetLow, dtSeconds)
        mid = smoothEnvelopeStep(mid, targetMid, dtSeconds)
        high = smoothEnvelopeStep(high, targetHigh, dtSeconds)

        val energyBoost = ((low + mid * 1.2f + high * 0.8f) / 3f) * 3.8f
        phaseRad = (phaseRad + (baseAngularSpeed + energyBoost) * dtSeconds) % (200f * PI.toFloat())
    }
}

private data class RibbonLayerSpec(
    val cycles: Float,
    val phaseMultiplier: Float,
    val phaseOffsetRad: Float,
    val amplitudeScale: Float,
    val idleFloorScale: Float,
    val strokeDp: Dp,
)

private val ThreeRibbonLayers = listOf(
    // Layer 0 (Back): Broad low-frequency foundation
    RibbonLayerSpec(
        cycles = 1.45f,
        phaseMultiplier = 0.78f,
        phaseOffsetRad = 0.0f,
        amplitudeScale = 0.86f,
        idleFloorScale = 0.78f,
        strokeDp = 1.1.dp,
    ),
    // Layer 1 (Hero): Mid-band vocal presence
    RibbonLayerSpec(
        cycles = 2.25f,
        phaseMultiplier = -1.0f,
        phaseOffsetRad = 1.7f,
        amplitudeScale = 1.0f,
        idleFloorScale = 1.0f,
        strokeDp = 1.6.dp,
    ),
    // Layer 2 (Front): Crisp high-frequency shimmer
    RibbonLayerSpec(
        cycles = 3.45f,
        phaseMultiplier = 1.32f,
        phaseOffsetRad = 3.1f,
        amplitudeScale = 0.72f,
        idleFloorScale = 0.55f,
        strokeDp = 1.15.dp,
    ),
)

/**
 * Fluid 3-layer symmetric audio ribbon visualizer driven by 3-band PCM energy
 * (`low`, `mid`, `high`) at display refresh rate via `withFrameNanos`.
 */
@Composable
fun SonaWaveform(
    audioBands: AudioEnergyBands,
    mode: WaveformVisualMode,
    modifier: Modifier = Modifier,
    height: Dp = 64.dp,
    peakCenterFraction: Float = 0.5f,
    recordingColor: Color = LocalSonaRecordingColor.current,
    primaryColor: Color = MaterialTheme.colorScheme.primary,
    tertiaryColor: Color = MaterialTheme.colorScheme.tertiary,
    mutedColor: Color = MaterialTheme.colorScheme.outlineVariant,
) {
    val activeWeight by animateFloatAsState(
        targetValue = when (mode) {
            WaveformVisualMode.ACTIVE -> 1f
            WaveformVisualMode.PREPARING -> 0.55f
            WaveformVisualMode.PAUSED -> 0.25f
            WaveformVisualMode.IDLE -> 0.18f
        },
        animationSpec = spring(dampingRatio = 0.85f, stiffness = 320f),
        label = "waveformModeWeight",
    )

    val currentBands by rememberUpdatedState(audioBands)
    val currentMode by rememberUpdatedState(mode)
    val physics = remember { WaveformPhysicsState() }
    var drawFrameNanos by remember { mutableLongStateOf(0L) }

    LaunchedEffect(Unit) {
        var lastNanos = 0L
        while (true) {
            withFrameNanos { frameNanos ->
                val dt = if (lastNanos == 0L) {
                    0.016f
                } else {
                    ((frameNanos - lastNanos) / 1_000_000_000f).coerceIn(0f, 0.05f)
                }
                lastNanos = frameNanos
                physics.step(
                    targetBands = currentBands,
                    mode = currentMode,
                    dtSeconds = dt,
                )
                drawFrameNanos = frameNanos
            }
        }
    }

    val pointCount = 56
    val yOffsets = remember { FloatArray(pointCount) }
    val layerPaths = remember { List(ThreeRibbonLayers.size) { Path() } }

    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(height),
    ) {
        // Read snapshot state in draw phase so animation never triggers recomposition
        @Suppress("UNUSED_VARIABLE")
        val frameTick = drawFrameNanos
        drawRibbonLayers(
            layers = ThreeRibbonLayers,
            layerEnergies = floatArrayOf(physics.low, physics.mid, physics.high),
            layerPaths = layerPaths,
            yOffsets = yOffsets,
            phaseRad = physics.phaseRad,
            mode = currentMode,
            activeWeight = activeWeight,
            peakCenterFraction = peakCenterFraction,
            recordingColor = recordingColor,
            primaryColor = primaryColor,
            tertiaryColor = tertiaryColor,
            mutedColor = mutedColor,
            idleFloorPx = 3.2.dp.toPx(),
            activeSilenceFloorPx = 4.2.dp.toPx(),
        )
    }
}

/**
 * Compact 3-layer fluid wave indicator for background mini-bars (`RecordMiniBar`).
 */
@Composable
fun SonaMiniWaveform(
    audioBands: AudioEnergyBands,
    mode: WaveformVisualMode,
    modifier: Modifier = Modifier,
    width: Dp = 36.dp,
    height: Dp = 24.dp,
    recordingColor: Color = LocalSonaRecordingColor.current,
    primaryColor: Color = MaterialTheme.colorScheme.primary,
    tertiaryColor: Color = MaterialTheme.colorScheme.tertiary,
    mutedColor: Color = MaterialTheme.colorScheme.outlineVariant,
) {
    val activeWeight by animateFloatAsState(
        targetValue = if (mode == WaveformVisualMode.ACTIVE) 1f else 0.35f,
        animationSpec = spring(dampingRatio = 0.85f, stiffness = 360f),
        label = "miniWaveformModeWeight",
    )

    val currentBands by rememberUpdatedState(audioBands)
    val currentMode by rememberUpdatedState(mode)
    val physics = remember { WaveformPhysicsState() }
    var drawFrameNanos by remember { mutableLongStateOf(0L) }

    LaunchedEffect(Unit) {
        var lastNanos = 0L
        while (true) {
            withFrameNanos { frameNanos ->
                val dt = if (lastNanos == 0L) {
                    0.016f
                } else {
                    ((frameNanos - lastNanos) / 1_000_000_000f).coerceIn(0f, 0.05f)
                }
                lastNanos = frameNanos
                physics.step(
                    targetBands = currentBands,
                    mode = currentMode,
                    dtSeconds = dt,
                )
                drawFrameNanos = frameNanos
            }
        }
    }

    val pointCount = 24
    val yOffsets = remember { FloatArray(pointCount) }
    val layerPaths = remember { List(ThreeRibbonLayers.size) { Path() } }

    Canvas(modifier = modifier.size(width = width, height = height)) {
        @Suppress("UNUSED_VARIABLE")
        val frameTick = drawFrameNanos
        drawRibbonLayers(
            layers = ThreeRibbonLayers,
            layerEnergies = floatArrayOf(physics.low, physics.mid, physics.high),
            layerPaths = layerPaths,
            yOffsets = yOffsets,
            phaseRad = physics.phaseRad,
            mode = currentMode,
            activeWeight = activeWeight,
            peakCenterFraction = 0.5f,
            recordingColor = recordingColor,
            primaryColor = primaryColor,
            tertiaryColor = tertiaryColor,
            mutedColor = mutedColor,
            idleFloorPx = 2.0.dp.toPx(),
            activeSilenceFloorPx = 2.8.dp.toPx(),
        )
    }
}

private fun DrawScope.drawRibbonLayers(
    layers: List<RibbonLayerSpec>,
    layerEnergies: FloatArray,
    layerPaths: List<Path>,
    yOffsets: FloatArray,
    phaseRad: Float,
    mode: WaveformVisualMode,
    activeWeight: Float,
    peakCenterFraction: Float,
    recordingColor: Color,
    primaryColor: Color,
    tertiaryColor: Color,
    mutedColor: Color,
    idleFloorPx: Float,
    activeSilenceFloorPx: Float,
) {
    val totalWidth = size.width
    val totalHeight = size.height
    if (totalWidth <= 0f || totalHeight <= 0f) return

    val centerY = totalHeight * 0.5f
    val maxHalfHeight = totalHeight * 0.44f
    val pointCount = yOffsets.size
    val lastIdx = pointCount - 1

    val breathingBias = (0.85f + 0.15f * sin(phaseRad * 0.65f))
    val baseFloorPx = when (mode) {
        WaveformVisualMode.ACTIVE -> activeSilenceFloorPx * breathingBias
        WaveformVisualMode.PREPARING -> activeSilenceFloorPx * 0.9f
        WaveformVisualMode.PAUSED -> idleFloorPx * 0.72f
        WaveformVisualMode.IDLE -> idleFloorPx * breathingBias
    }

    for (layerIndex in layers.indices) {
        val spec = layers[layerIndex]
        val energy = layerEnergies[layerIndex]
        val amplitudePx = (baseFloorPx * spec.idleFloorScale +
            energy * (maxHalfHeight - baseFloorPx) * spec.amplitudeScale)
            .coerceIn(1f, maxHalfHeight)

        val layerPhase = phaseRad * spec.phaseMultiplier + spec.phaseOffsetRad
        for (i in 0 until pointCount) {
            val fraction = i.toFloat() / lastIdx.toFloat()
            yOffsets[i] = evaluateWaveDisplacement(
                fraction = fraction,
                cycles = spec.cycles,
                phaseRad = layerPhase,
                amplitudePx = amplitudePx,
                peakCenterFraction = peakCenterFraction,
            )
        }

        val path = layerPaths[layerIndex]
        buildSymmetricRibbonPath(
            path = path,
            yOffsets = yOffsets,
            totalWidth = totalWidth,
            centerY = centerY,
        )

        val (fillTop, fillCenter, rimColor) = resolveLayerColors(
            layerIndex = layerIndex,
            activeWeight = activeWeight,
            mode = mode,
            recordingColor = recordingColor,
            primaryColor = primaryColor,
            tertiaryColor = tertiaryColor,
            mutedColor = mutedColor,
        )

        drawPath(
            path = path,
            brush = Brush.verticalGradient(
                0.0f to fillTop,
                0.5f to fillCenter,
                1.0f to fillTop,
                startY = centerY - amplitudePx,
                endY = centerY + amplitudePx,
            ),
        )

        drawPath(
            path = path,
            color = rimColor,
            style = Stroke(
                width = spec.strokeDp.toPx(),
                cap = StrokeCap.Round,
                join = StrokeJoin.Round,
            ),
        )
    }
}

private fun resolveLayerColors(
    layerIndex: Int,
    activeWeight: Float,
    mode: WaveformVisualMode,
    recordingColor: Color,
    primaryColor: Color,
    tertiaryColor: Color,
    mutedColor: Color,
): Triple<Color, Color, Color> {
    val idleTint = lerp(mutedColor, primaryColor, 0.38f)
    val pausedTint = lerp(mutedColor, tertiaryColor, 0.55f)
    val inactiveBase = if (mode == WaveformVisualMode.PAUSED) pausedTint else idleTint

    return when (layerIndex) {
        // Layer 0 (Low / Back): primary-anchored soft foundation
        0 -> {
            val base = lerp(inactiveBase, primaryColor, activeWeight)
            Triple(
                base.copy(alpha = 0.16f + 0.18f * activeWeight),
                base.copy(alpha = 0.06f + 0.09f * activeWeight),
                base.copy(alpha = 0.28f + 0.24f * activeWeight),
            )
        }
        // Layer 1 (Mid / Hero): recording accent blended with primary
        1 -> {
            val hero = lerp(inactiveBase, lerp(recordingColor, primaryColor, 0.18f), activeWeight)
            Triple(
                hero.copy(alpha = 0.24f + 0.30f * activeWeight),
                hero.copy(alpha = 0.08f + 0.14f * activeWeight),
                hero.copy(alpha = 0.38f + 0.54f * activeWeight),
            )
        }
        // Layer 2 (High / Shimmer): tertiary & recording highlight
        else -> {
            val shimmer = lerp(inactiveBase, lerp(tertiaryColor, recordingColor, 0.42f), activeWeight)
            Triple(
                shimmer.copy(alpha = 0.15f + 0.22f * activeWeight),
                shimmer.copy(alpha = 0.05f + 0.08f * activeWeight),
                shimmer.copy(alpha = 0.28f + 0.46f * activeWeight),
            )
        }
    }
}

private fun buildSymmetricRibbonPath(
    path: Path,
    yOffsets: FloatArray,
    totalWidth: Float,
    centerY: Float,
) {
    path.reset()
    val count = yOffsets.size
    if (count < 2) return
    val stepX = totalWidth / (count - 1).toFloat()

    path.moveTo(0f, centerY - yOffsets[0])
    for (i in 1 until count - 1) {
        val prevX = i * stepX
        val prevY = centerY - yOffsets[i]
        val nextX = (i + 1) * stepX
        val nextY = centerY - yOffsets[i + 1]
        path.quadraticTo(
            prevX,
            prevY,
            (prevX + nextX) * 0.5f,
            (prevY + nextY) * 0.5f,
        )
    }
    path.lineTo(totalWidth, centerY - yOffsets[count - 1])

    for (i in count - 2 downTo 1) {
        val prevX = i * stepX
        val prevY = centerY + yOffsets[i]
        val nextX = (i - 1) * stepX
        val nextY = centerY + yOffsets[i - 1]
        path.quadraticTo(
            prevX,
            prevY,
            (prevX + nextX) * 0.5f,
            (prevY + nextY) * 0.5f,
        )
    }
    path.lineTo(0f, centerY + yOffsets[0])
    path.close()
}
