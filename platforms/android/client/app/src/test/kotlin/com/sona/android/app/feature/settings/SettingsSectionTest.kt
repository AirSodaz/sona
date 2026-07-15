package com.sona.android.app.feature.settings

import androidx.compose.material3.adaptive.layout.ListDetailPaneScaffoldRole
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
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

    @Test
    @OptIn(ExperimentalMaterial3AdaptiveApi::class)
    fun `recognition deep links initialize list and detail navigation history`() {
        val history = settingsDestinationHistory(SettingsSection.RECOGNITION)

        assertEquals(
            listOf(
                ListDetailPaneScaffoldRole.List,
                ListDetailPaneScaffoldRole.Detail,
            ),
            history.map { it.pane },
        )
        assertNull(history.first().contentKey)
        assertEquals(SettingsSection.RECOGNITION, history.last().contentKey)
    }
}
