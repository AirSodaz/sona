package com.sona.android.adapters.android.settings

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalAsrModelDownloadStorageTest {
    @Test
    fun `download storage keeps extraction copy and safety margin`() {
        val estimated = 512L * 1_024 * 1_024
        val required = estimated * 2 + 128L * 1_024 * 1_024

        assertFalse(hasLocalModelDownloadStorage(required - 1, estimated))
        assertTrue(hasLocalModelDownloadStorage(required, estimated))
    }

    @Test
    fun `unknown model size does not block download`() {
        assertTrue(hasLocalModelDownloadStorage(0, 0))
    }

    @Test
    fun `overflowing estimates are rejected`() {
        assertFalse(hasLocalModelDownloadStorage(Long.MAX_VALUE, Long.MAX_VALUE))
    }
}
