package com.sona.android.adapters.android.settings

import java.io.File
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class LocalAsrModelStorageValidationTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun `sha256 verification rejects mismatched downloads`() {
        val file = temporaryFolder.newFile("model.onnx").apply { writeText("hello") }

        verifySha256(
            file,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        )
        assertThrows(IOException::class.java) {
            verifySha256(file, "0".repeat(64))
        }
    }

    @Test
    fun `archive destinations remain inside the install directory`() {
        val root = temporaryFolder.newFolder("install")

        assertEquals(
            File(root, "model/tokens.txt").canonicalFile,
            safeArchiveDestination(root, "model/tokens.txt"),
        )
        assertThrows(IllegalArgumentException::class.java) {
            safeArchiveDestination(root, "../outside.onnx")
        }
        assertThrows(IllegalArgumentException::class.java) {
            safeArchiveDestination(root, "model\\outside.onnx")
        }
    }
}
