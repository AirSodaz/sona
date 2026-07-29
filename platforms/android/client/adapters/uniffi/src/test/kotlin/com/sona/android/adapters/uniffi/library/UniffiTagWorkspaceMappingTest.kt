package com.sona.android.adapters.uniffi.library

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiTagRecordV1

class UniffiTagWorkspaceMappingTest {
    @Test
    fun `maps typed tag records`() {
        val tag = FfiTagRecordV1("tag-1", "Work", "Meetings", "briefcase", "#123456", 4uL, 5uL, 6uL)

        assertEquals("Work", tag.toApplication().name)
        assertEquals(4, tag.toApplication().sortOrder)
    }

    @Test
    fun `rejects tag numbers outside Android Long range`() {
        assertThrows(IllegalArgumentException::class.java) {
            FfiTagRecordV1("tag-1", "Work", "", "", "", ULong.MAX_VALUE, 0uL, 0uL).toApplication()
        }
    }
}
