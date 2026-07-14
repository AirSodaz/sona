package com.sona.android.app.feature.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SettingsSectionTest {
    @Test
    fun `stable routes resolve to their settings sections`() {
        assertEquals(SettingsSection.APPEARANCE, SettingsSection.fromRoute("appearance"))
        assertEquals(SettingsSection.RECOGNITION, SettingsSection.fromRoute("recognition"))
    }

    @Test
    fun `missing and unknown routes do not select a section`() {
        assertNull(SettingsSection.fromRoute(null))
        assertNull(SettingsSection.fromRoute("unknown"))
    }
}
