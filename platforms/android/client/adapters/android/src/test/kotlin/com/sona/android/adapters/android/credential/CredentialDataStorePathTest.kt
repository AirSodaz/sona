package com.sona.android.adapters.android.credential

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class CredentialDataStorePathTest {
    @Test
    fun `storage file is confined to a canonical direct child`() {
        val root = Files.createTempDirectory("sona-credential-path").toFile()
        val noBackupDirectory = File(root, "no-backup").apply { mkdirs() }
        try {
            val resolved = resolveCredentialStorageFile(
                directory = noBackupDirectory,
                fileName = "streaming_credentials.preferences_pb",
            )

            assertEquals(
                File(noBackupDirectory.canonicalFile, "streaming_credentials.preferences_pb"),
                resolved,
            )
            listOf("", " ", ".", "..", "../outside.preferences_pb", "nested/file.preferences_pb")
                .forEach { invalidName ->
                    assertThrows(IllegalArgumentException::class.java) {
                        resolveCredentialStorageFile(noBackupDirectory, invalidName)
                    }
                }
        } finally {
            root.deleteRecursively()
        }
    }
}
