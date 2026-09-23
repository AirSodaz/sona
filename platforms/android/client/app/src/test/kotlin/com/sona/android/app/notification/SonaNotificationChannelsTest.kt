package com.sona.android.app.notification

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SonaNotificationChannelsTest {
    @Test
    fun `channel constants are distinct and match expected ids`() {
        assertEquals("recording", SonaNotificationChannels.CHANNEL_RECORDING)
        assertEquals("audio_imports", SonaNotificationChannels.CHANNEL_AUDIO_IMPORTS)
        assertEquals("tasks", SonaNotificationChannels.CHANNEL_TASKS)

        val channelIds = setOf(
            SonaNotificationChannels.CHANNEL_RECORDING,
            SonaNotificationChannels.CHANNEL_AUDIO_IMPORTS,
            SonaNotificationChannels.CHANNEL_TASKS,
        )
        assertEquals(3, channelIds.size)
        assertTrue(channelIds.all { it.isNotBlank() })
    }
}
