package com.sona.android.adapters.android.system

import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class UuidRecordingIdPortTest {
    @Test
    fun `UUID supplier remains injectable and produces canonical text`() {
        val expected = UUID.fromString("123E4567-E89B-12D3-A456-426614174000")
        val ids = UuidRecordingIdPort(uuidSupplier = { expected })

        assertEquals("123e4567-e89b-12d3-a456-426614174000", ids.nextRecordingId())
    }

    @Test
    fun `default supplier returns distinct parseable UUIDs`() {
        val ids = UuidRecordingIdPort()

        val first = ids.nextRecordingId()
        val second = ids.nextRecordingId()

        assertEquals(first, UUID.fromString(first).toString())
        assertEquals(second, UUID.fromString(second).toString())
        assertNotEquals(first, second)
    }
}
