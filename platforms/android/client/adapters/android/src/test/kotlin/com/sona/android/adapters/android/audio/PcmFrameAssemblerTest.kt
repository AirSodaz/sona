package com.sona.android.adapters.android.audio

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class PcmFrameAssemblerTest {
    @Test
    fun `split reads become exact independent frames`() {
        val assembler = PcmFrameAssembler(frameSizeBytes = 640)
        val expected = ByteArray(1_280) { index -> (index % 251).toByte() }

        val first = assembler.append(expected, 0, 100)
        val second = assembler.append(expected, 100, 1_180)

        assertEquals(emptyList<ByteArray>(), first)
        assertEquals(2, second.size)
        assertArrayEquals(expected.copyOfRange(0, 640), second[0])
        assertArrayEquals(expected.copyOfRange(640, 1_280), second[1])
        expected.fill(0)
        assertEquals(2, second.distinctBy(System::identityHashCode).size)
        assertEquals(640, second[0].size)
        assertEquals(640, second[1].size)
    }
}
