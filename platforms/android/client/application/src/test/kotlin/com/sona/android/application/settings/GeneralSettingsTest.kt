package com.sona.android.application.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class GeneralSettingsTest {
    @Test
    fun `mirror resolution orders GitHub candidates according to strategy`() {
        val url = "https://github.com/AirSodaz/sona/releases/latest/download/android-update.json"

        assertEquals(
            listOf(
                url,
                "https://ghproxy.net/$url",
                "https://mirror.ghproxy.com/$url",
            ),
            DownloadUrlResolver.candidatesForGitHubUrl(url, DownloadMirror.AUTO),
        )
        assertEquals(
            listOf(url),
            DownloadUrlResolver.candidatesForGitHubUrl(url, DownloadMirror.DIRECT),
        )
        assertEquals(
            listOf(
                "https://mirror.ghproxy.com/$url",
                url,
                "https://ghproxy.net/$url",
            ),
            DownloadUrlResolver.candidatesForGitHubUrl(url, DownloadMirror.GH_PROXY),
        )
        assertEquals(
            listOf(
                "https://ghproxy.net/$url",
                url,
                "https://mirror.ghproxy.com/$url",
            ),
            DownloadUrlResolver.candidatesForGitHubUrl(url, DownloadMirror.GH_NET),
        )
    }

    @Test
    fun `log level priority filtering enforces threshold`() {
        assertTrue(AppLogLevel.INFO.allows(AppLogLevel.INFO))
        assertTrue(AppLogLevel.INFO.allows(AppLogLevel.ERROR))
        assertFalse(AppLogLevel.INFO.allows(AppLogLevel.DEBUG))
        assertTrue(AppLogLevel.TRACE.allows(AppLogLevel.DEBUG))
    }

    @Test
    fun `wire values parse safely with fallback`() {
        assertEquals(DownloadMirror.GH_NET, DownloadMirror.fromWireValue("ghnet"))
        assertEquals(DownloadMirror.AUTO, DownloadMirror.fromWireValue("unknown"))
        assertEquals(AppLogLevel.WARN, AppLogLevel.fromWireValue("WARN"))
        assertEquals(AppLogLevel.INFO, AppLogLevel.fromWireValue("invalid"))
    }
}
