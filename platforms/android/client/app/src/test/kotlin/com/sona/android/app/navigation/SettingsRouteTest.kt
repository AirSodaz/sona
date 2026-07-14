package com.sona.android.app.navigation

import com.sona.android.app.feature.settings.SettingsSection
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingsRouteTest {
    @Test
    fun `recognition configuration uses the settings section route`() {
        assertEquals("settings?section=recognition", settingsRoute(SettingsSection.RECOGNITION))
    }

    @Test
    fun `settings destination matches its base and argument route`() {
        assertTrue(SonaDestination.SETTINGS.matches("settings"))
        assertTrue(SonaDestination.SETTINGS.matches("settings?section={section}"))
        assertFalse(SonaDestination.SETTINGS.matches("record"))
    }
}
