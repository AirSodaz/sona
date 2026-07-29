package com.sona.android.adapters.android.data

import java.io.File
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class FilePathPolicyTest {
    @get:Rule val temporaryFolder = TemporaryFolder()

    @Test
    fun `accepts descendants and rejects sibling prefix paths`() {
        val root = temporaryFolder.newFolder("exports")
        val child = File(root, "nested/export.txt").apply { parentFile?.mkdirs() }
        val sibling = temporaryFolder.newFolder("exports-escaped")

        assertTrue(isFileWithinRoot(child, root))
        assertFalse(isFileWithinRoot(sibling, root))
        assertFalse(isFileWithinRoot(root, root))
    }
}
