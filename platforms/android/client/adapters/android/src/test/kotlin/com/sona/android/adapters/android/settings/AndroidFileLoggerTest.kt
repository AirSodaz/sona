package com.sona.android.adapters.android.settings

import com.sona.android.application.settings.AppLogLevel
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class AndroidFileLoggerTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `filters by log level, rotates on size limit, and clears logs`() = runTest {
        val dir = temporaryFolder.newFolder("logs")
        var callbackLevel: AppLogLevel? = null
        val logger = AndroidFileLogger(
            logsDir = dir,
            initialLevel = AppLogLevel.WARN,
            maxFileBytes = 120L,
            onLevelChanged = { callbackLevel = it },
        )

        logger.append("info", "test", "ignored info line")
        logger.append("warn", "test", "first warn line " + "x".repeat(120))
        logger.append("error", "test", "second error line")

        val text = logger.readRecentLogs()
        assertFalse(text.contains("ignored info line"))
        assertTrue(text.contains("first warn line"))
        assertTrue(text.contains("second error line"))
        assertTrue(logger.logSizeBytes() > 0L)

        logger.applyLogLevel(AppLogLevel.DEBUG)
        assertEquals(AppLogLevel.DEBUG, callbackLevel)

        logger.clearLogs()
        assertEquals(0L, logger.logSizeBytes())
    }
}
