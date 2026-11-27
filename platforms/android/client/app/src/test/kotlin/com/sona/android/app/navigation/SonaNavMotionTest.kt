package com.sona.android.app.navigation

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SonaNavMotionTest {

    @Test
    fun `root tab identification recognizes library and settings root`() {
        assertTrue(SonaNavMotion.isRootTab("library"))
        assertTrue(SonaNavMotion.isRootTab("settings"))
        assertTrue(SonaNavMotion.isRootTab("settings?section={section}", sectionArg = null))
        assertTrue(SonaNavMotion.isRootTab("settings?section={section}", sectionArg = ""))

        assertFalse(SonaNavMotion.isRootTab("home"))
        assertFalse(SonaNavMotion.isRootTab("settings?section=recognition"))
        assertFalse(SonaNavMotion.isRootTab("settings?section=data_recovery"))
        assertFalse(SonaNavMotion.isRootTab("settings?section={section}", sectionArg = "recognition"))
        assertFalse(SonaNavMotion.isRootTab("library/item-1"))
        assertFalse(SonaNavMotion.isRootTab(LIBRARY_DETAIL_ROUTE))
        assertFalse(SonaNavMotion.isRootTab(null))
    }

    @Test
    fun `root tab index returns correct sequential indices`() {
        assertEquals(0, SonaNavMotion.rootTabIndex("library"))
        assertEquals(1, SonaNavMotion.rootTabIndex("settings"))
        assertEquals(1, SonaNavMotion.rootTabIndex("settings?section={section}", sectionArg = null))

        assertEquals(-1, SonaNavMotion.rootTabIndex("home"))
        assertEquals(-1, SonaNavMotion.rootTabIndex("settings?section=recognition"))
        assertEquals(-1, SonaNavMotion.rootTabIndex("settings?section={section}", sectionArg = "recognition"))
        assertEquals(-1, SonaNavMotion.rootTabIndex("other"))
        assertEquals(-1, SonaNavMotion.rootTabIndex(null))
    }

    @Test
    fun `sub-page identification recognizes concrete library detail and settings sections`() {
        assertTrue(SonaNavMotion.isSubPage(LIBRARY_DETAIL_ROUTE))
        assertTrue(SonaNavMotion.isSubPage("library/history-abc"))
        assertTrue(SonaNavMotion.isSubPage("settings?section=recognition"))
        assertTrue(SonaNavMotion.isSubPage("settings?section=data_recovery"))
        assertTrue(SonaNavMotion.isSubPage("settings?section={section}", sectionArg = "recognition"))

        assertFalse(SonaNavMotion.isSubPage("library"))
        assertFalse(SonaNavMotion.isSubPage("settings"))
        assertFalse(SonaNavMotion.isSubPage("settings?section={section}", sectionArg = null))
        assertFalse(SonaNavMotion.isSubPage(null))
    }

    @Test
    fun `peer tab switches resolve directional horizontal sliding without jumping`() {
        assertEquals(
            NavMotionDirection.FORWARD,
            SonaNavMotion.resolveMotionDirection("library", "settings", isPop = false),
        )
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection("settings", "library", isPop = false),
        )
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection(
                "settings?section={section}",
                "library",
                initialSectionArg = null,
                isPop = true,
            ),
        )
    }

    @Test
    fun `drill-down to subpages and settings sections resolves forward slide`() {
        assertEquals(
            NavMotionDirection.FORWARD,
            SonaNavMotion.resolveMotionDirection("library", "library/history-1", isPop = false),
        )
        assertEquals(
            NavMotionDirection.FORWARD,
            SonaNavMotion.resolveMotionDirection("library", LIBRARY_DETAIL_ROUTE, isPop = false),
        )
        assertEquals(
            NavMotionDirection.FORWARD,
            SonaNavMotion.resolveMotionDirection("library", "settings?section=recognition", isPop = false),
        )
        assertEquals(
            NavMotionDirection.FORWARD,
            SonaNavMotion.resolveMotionDirection(
                "library",
                "settings?section={section}",
                targetSectionArg = "recognition",
                isPop = false,
            ),
        )
    }

    @Test
    fun `returning from subpages and between subpages resolves backward slide`() {
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection("library/history-1", "library", isPop = true),
        )
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection(LIBRARY_DETAIL_ROUTE, "library", isPop = true),
        )
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection("settings?section=recognition", "library", isPop = true),
        )
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection(
                "settings?section={section}",
                "library",
                initialSectionArg = "recognition",
                isPop = true,
            ),
        )
        // Subpage-to-subpage POP: must always be BACKWARD
        assertEquals(
            NavMotionDirection.BACKWARD,
            SonaNavMotion.resolveMotionDirection(
                "library/history-1",
                "settings?section={section}",
                targetSectionArg = "data_recovery",
                isPop = true,
            ),
        )
    }

    @Test
    fun `identical route resolves none`() {
        assertEquals(
            NavMotionDirection.NONE,
            SonaNavMotion.resolveMotionDirection("library", "library"),
        )
        assertEquals(
            NavMotionDirection.NONE,
            SonaNavMotion.resolveMotionDirection(
                "settings?section={section}",
                "settings?section={section}",
                initialSectionArg = "recognition",
                targetSectionArg = "recognition",
            ),
        )
    }

    @Test
    fun `peer tab transition detection matches root library and settings switches only`() {
        assertTrue(SonaNavMotion.isPeerTabTransition("library", "settings"))
        assertTrue(
            SonaNavMotion.isPeerTabTransition(
                "settings?section={section}",
                "library",
                initialSectionArg = null,
            ),
        )
        assertTrue(
            SonaNavMotion.isPeerTabTransition(
                "library",
                "settings?section={section}",
                targetSectionArg = "",
            ),
        )

        assertFalse(SonaNavMotion.isPeerTabTransition("library", "library"))
        assertFalse(SonaNavMotion.isPeerTabTransition("library", LIBRARY_DETAIL_ROUTE))
        assertFalse(
            SonaNavMotion.isPeerTabTransition(
                "settings?section={section}",
                "settings?section={section}",
                initialSectionArg = null,
                targetSectionArg = "recognition",
            ),
        )
        assertFalse(
            SonaNavMotion.isPeerTabTransition(
                "library",
                "settings?section={section}",
                targetSectionArg = "recognition",
            ),
        )
    }
}
