package com.sona.android.adapters.android.settings

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class LocalAsrModelDetectionTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `detects sensevoice and its sibling VAD recursively`() {
        val root = temporaryFolder.newFolder("models")
        val model = File(root, "sensevoice-int8").apply { mkdir() }
        File(model, "model.int8.onnx").writeText("model")
        File(model, "tokens.txt").writeText("tokens")
        File(root, "silero_vad.onnx").writeText("vad")

        val detected = checkNotNull(detectLocalAsrModel(root))

        assertEquals("sensevoice", detected.modelType)
        assertEquals("model.int8.onnx", detected.files.model)
        assertEquals("tokens.txt", detected.files.tokens)
        assertEquals(File(root, "silero_vad.onnx"), detected.vadModel)
        assertTrue(detected.requiresVad)
    }

    @Test
    fun `distinguishes paraformer and zipformer by the joiner file`() {
        val paraformer = temporaryFolder.newFolder("paraformer")
        listOf("encoder.onnx", "decoder.onnx", "tokens.txt").forEach {
            File(paraformer, it).writeText(it)
        }
        val paraformerResult = checkNotNull(detectLocalAsrModel(paraformer))
        assertEquals("paraformer", paraformerResult.modelType)
        assertFalse(paraformerResult.requiresVad)

        val zipformer = temporaryFolder.newFolder("zipformer")
        listOf("encoder.int8.onnx", "decoder.onnx", "joiner.int8.onnx", "tokens.txt")
            .forEach { File(zipformer, it).writeText(it) }
        val zipformerResult = checkNotNull(detectLocalAsrModel(zipformer))
        assertEquals("zipformer", zipformerResult.modelType)
        assertEquals("joiner.int8.onnx", zipformerResult.files.joiner)
        assertFalse(zipformerResult.requiresVad)
    }

    @Test
    fun `rejects folders without a complete supported model`() {
        val root = temporaryFolder.newFolder("incomplete")
        File(root, "model.int8.onnx").writeText("model")

        assertNull(detectLocalAsrModel(root))
    }
}
