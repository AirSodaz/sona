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

    @Test
    fun `android model sources are strictly int8 small models`() {
        val repo = AndroidModelSourceRepository()
        val sources = repo.loadSources()
        assert(sources.isNotEmpty())
        for (source in sources) {
            assertEquals("int8", source.quantization.lowercase())
            assert(parseSizeBytes(source.size) > 0)
        }
        val recommended = sources.filter { it.isRecommended }
        assertEquals(1, recommended.size)
        assertEquals("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17", recommended.first().id)
    }

    @Test
    fun `filters out non-int8 models if present in raw sources`() {
        val raw = """
            {
              "version": "1.0.0",
              "models": [
                { "id": "m1", "name": "M1", "modelType": "test", "quantization": "int8", "size": "100 MB" },
                { "id": "m2", "name": "M2", "modelType": "test", "quantization": "fp32", "size": "1 GB" },
                { "id": "m3", "name": "M3", "modelType": "test", "quantization": "fp16", "size": "500 MB" }
              ]
            }
        """.trimIndent()
        val repo = AndroidModelSourceRepository { raw.byteInputStream() }
        val sources = repo.loadSources()
        assertEquals(1, sources.size)
        assertEquals("m1", sources[0].id)
    }
}
