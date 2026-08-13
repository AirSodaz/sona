package com.sona.android.application.llm

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LlmTest {
    @Test fun `configuration requires valid endpoint model and key`() {
        val config = LlmConfig()
        assertTrue(config.validate("secret"))
        assertFalse(config.validate(null))
        assertFalse(config.copy(baseUrl = "ftp://example.test").validate("secret"))
        assertFalse(config.copy(model = " ").validate("secret"))
    }

    @Test fun `progress clamps to valid percentage`() {
        assertEquals(0, LlmTaskProgress(0, 0).percent)
        assertEquals(50, LlmTaskProgress(1, 2).percent)
        assertEquals(100, LlmTaskProgress(4, 2).percent)
    }
}
