package com.sona.android.app.navigation

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SonaFloatingDockTest {

    @Test
    fun `floating dock is shown on root tabs`() {
        assertTrue(SonaFloatingDockDefaults.shouldShow("library"))
        assertTrue(SonaFloatingDockDefaults.shouldShow("settings"))
        assertTrue(SonaFloatingDockDefaults.shouldShow("settings?section={section}", sectionArg = null))
        assertTrue(SonaFloatingDockDefaults.shouldShow("settings?section={section}", sectionArg = ""))
    }

    @Test
    fun `floating dock is hidden on sub-pages and detail workspaces`() {
        assertFalse(SonaFloatingDockDefaults.shouldShow("home"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("library/rec-456"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("library/{historyId}"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("settings?section=appearance"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("settings?section=sync"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("settings?section=recognition"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("settings?section=llm"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("settings?section=about"))
        assertFalse(SonaFloatingDockDefaults.shouldShow("settings?section={section}", sectionArg = "appearance"))
        assertFalse(SonaFloatingDockDefaults.shouldShow(null))
    }

    @Test
    fun `floating dock design specs conform to M3 Expressive guidelines`() {
        assertEquals(RoundedCornerShape(32.dp), SonaFloatingDockDefaults.ContainerShape)
        assertEquals(RoundedCornerShape(28.dp), SonaFloatingDockDefaults.CaptureShape)
        assertEquals(6.dp, SonaFloatingDockDefaults.Elevation)
        assertEquals(16.dp, SonaFloatingDockDefaults.HorizontalMargin)
        assertEquals(12.dp, SonaFloatingDockDefaults.BottomMargin)
        assertEquals(480.dp, SonaFloatingDockDefaults.MaxWidth)
        assertEquals(62.dp, SonaFloatingDockDefaults.DockHeight)
    }
}
