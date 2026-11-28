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
        var callbackCount = 0
        var callbackLevel: AppLogLevel? = null
        val logger = AndroidFileLogger(
            logsDir = dir,
            initialLevel = AppLogLevel.WARN,
            maxFileBytes = 120L,
            onLevelChanged = {
                callbackCount += 1
                callbackLevel = it
            },
        )

        logger.append("info", "test", "ignored info line")
        logger.append("warn", "test", "first warn line " + "x".repeat(120))
        logger.append("error", "test", "second error line 你好")

        val text = logger.readRecentLogs()
        assertFalse(text.contains("ignored info line"))
        assertTrue(text.contains("first warn line"))
        assertTrue(text.contains("second error line 你好"))
        assertTrue(logger.logSizeBytes() > 0L)

        val tailed = logger.readRecentLogs(maxBytes = 12)
        assertFalse(tailed.contains('\uFFFD'))
        assertTrue(tailed.endsWith("你好\n"))

        logger.applyLogLevel(AppLogLevel.WARN)
        assertEquals(0, callbackCount)

        logger.applyLogLevel(AppLogLevel.DEBUG)
        logger.applyLogLevel(AppLogLevel.DEBUG)
        assertEquals(1, callbackCount)
        assertEquals(AppLogLevel.DEBUG, callbackLevel)

        logger.clearLogs()
        assertEquals(0L, logger.logSizeBytes())
    }
}
