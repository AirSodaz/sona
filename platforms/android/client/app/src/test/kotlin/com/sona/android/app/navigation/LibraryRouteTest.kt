package com.sona.android.app.navigation

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LibraryRouteTest {
    @Test
    fun `library detail route keeps the selected history id`() {
        assertEquals("library/history-1", libraryDetailRoute("history-1"))
    }

    @Test
    fun `library destination matches its list and detail routes`() {
        assertTrue(SonaDestination.LIBRARY.matches("library"))
        assertTrue(SonaDestination.LIBRARY.matches("library/{historyId}"))
        assertTrue(SonaDestination.LIBRARY.matches("library/history-1"))
        assertFalse(SonaDestination.LIBRARY.matches("record"))
    }
}
