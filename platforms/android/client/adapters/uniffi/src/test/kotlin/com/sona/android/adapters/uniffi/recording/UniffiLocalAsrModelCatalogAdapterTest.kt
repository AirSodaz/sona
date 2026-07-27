package com.sona.android.adapters.uniffi.recording

import org.junit.Assert.assertEquals
import org.junit.Test

class UniffiLocalAsrModelCatalogAdapterTest {
    @Test
    fun `parses catalog size labels to bytes`() {
        assertEquals(155L * 1_024 * 1_024, parseSizeBytes("~155 MB"))
        assertEquals((1.23 * 1_024 * 1_024 * 1_024).toLong(), parseSizeBytes("~1.23 GB"))
        assertEquals(629L * 1_024, parseSizeBytes("629KB"))
        assertEquals(0L, parseSizeBytes("unknown"))
    }
}
