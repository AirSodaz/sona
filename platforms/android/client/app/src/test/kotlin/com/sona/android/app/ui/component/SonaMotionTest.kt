package com.sona.android.app.ui.component

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

class SonaMotionTest {

    @Test
    fun `calculateDampedOverscroll provides initial movement at rest`() {
        val maxPx = 200f
        val delta = 50f
        val result = calculateDampedOverscroll(
            currentDisplacement = 0f,
            delta = delta,
            maxDisplacement = maxPx,
        )
        // With currentRatio = 0, resistance = 0.38f, result = 50 * 0.38 = 19.0f
        assertTrue(result > 0f)
        assertTrue(result < delta)
        assertEquals(19.0f, result, 0.01f)
    }

    @Test
    fun `calculateDampedOverscroll increases resistance as displacement grows`() {
        val maxPx = 200f
        val delta = 20f

        val earlyStep = calculateDampedOverscroll(
            currentDisplacement = 20f,
            delta = delta,
            maxDisplacement = maxPx,
        ) - 20f

        val lateStep = calculateDampedOverscroll(
            currentDisplacement = 180f,
            delta = delta,
            maxDisplacement = maxPx,
        ) - 180f

        assertTrue("Late movement ($lateStep) should have higher resistance than early movement ($earlyStep)", lateStep < earlyStep)
        assertTrue("Late step should still be positive", lateStep > 0f)
    }

    @Test
    fun `calculateDampedOverscroll respects boundary limits`() {
        val maxPx = 150f
        val hugeDelta = 1000f

        val clampedPositive = calculateDampedOverscroll(
            currentDisplacement = 140f,
            delta = hugeDelta,
            maxDisplacement = maxPx,
        )
        assertEquals(maxPx, clampedPositive, 0.001f)

        val clampedNegative = calculateDampedOverscroll(
            currentDisplacement = -140f,
            delta = -hugeDelta,
            maxDisplacement = maxPx,
        )
        assertEquals(-maxPx, clampedNegative, 0.001f)
    }

    @Test
    fun `card design tokens conform to 24dp inset-grouped specification`() {
        assertEquals(RoundedCornerShape(24.dp), SonaCardDefaults.CardShape)
        assertEquals(RoundedCornerShape(18.dp), SonaCardDefaults.SubGroupShape)
        assertEquals(2.dp, SonaCardDefaults.Elevation)
        assertEquals(56.dp, SonaCardDefaults.DividerIndent)
    }
}
