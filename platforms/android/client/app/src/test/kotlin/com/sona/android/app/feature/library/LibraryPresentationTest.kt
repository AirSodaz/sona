package com.sona.android.app.feature.library

import java.util.Locale
import java.util.TimeZone
import org.junit.Assert.assertEquals
import org.junit.Test

class LibraryPresentationTest {
    @Test
    fun `timestamp formatting is stable for an explicit locale and time zone`() {
        val utc = TimeZone.getTimeZone("UTC")

        assertEquals(
            "Jan 2, 2024, 3:04 AM",
            formatLibraryTimestamp(
                timestampEpochMillis = 1_704_164_640_000,
                locale = Locale.US,
                timeZone = utc,
            ).normalizeSpaces(),
        )
        assertEquals(
            "Jan 1, 1970, 12:00 AM",
            formatLibraryTimestamp(
                timestampEpochMillis = -1,
                locale = Locale.US,
                timeZone = utc,
            ).normalizeSpaces(),
        )
    }

    private fun String.normalizeSpaces(): String =
        replace('\u202f', ' ').replace('\u00a0', ' ')
}
