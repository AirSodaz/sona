package com.sona.android.adapters.uniffi.library

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiProjectRecordV1

class UniffiProjectWorkspaceMappingTest {
    @Test
    fun `maps typed project records`() {
        val project = FfiProjectRecordV1("project-1", "Work", "Meetings", "briefcase", "#123456", 4uL, 5uL, 6uL)

        assertEquals("Work", project.toApplication().name)
        assertEquals(4, project.toApplication().sortOrder)
    }

    @Test
    fun `rejects project numbers outside Android Long range`() {
        assertThrows(IllegalArgumentException::class.java) {
            FfiProjectRecordV1("project-1", "Work", "", "", "", ULong.MAX_VALUE, 0uL, 0uL).toApplication()
        }
    }
}
