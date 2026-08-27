package com.sona.android.adapters.android.system

import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class UuidRecordingIdPortTest {
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
