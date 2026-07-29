package com.sona.android.adapters.android.data

import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import com.sona.android.application.data.FileTransferPort
import java.io.File
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class AndroidSafFileTransferAdapter private constructor(
    private val context: Context,
) : FileTransferPort {
    private val importDir = File(context.cacheDir, "backup-imports")
    private val exportDir = File(context.cacheDir, "exports")

    override suspend fun stageImport(sourceUri: String): String = withContext(Dispatchers.IO) {
        val uri = requireContentUri(sourceUri)
        importDir.mkdirs()
        val destination = File.createTempFile("sona-backup-", ".tar.bz2", importDir)
        try {
            context.contentResolver.openInputStream(uri)?.use { input ->
                destination.outputStream().use(input::copyTo)
            } ?: throw IOException("Unable to open backup source.")
            destination.absolutePath
        } catch (error: Throwable) {
            destination.delete()
            throw error
        }
    }

    override suspend fun publishExport(stagedPath: String, destinationUri: String) =
        withContext(Dispatchers.IO) {
            val source = managedFile(stagedPath)
            require(source.isFile) { "Export staging file does not exist." }
            val uri = requireContentUri(destinationUri)
            context.contentResolver.openOutputStream(uri, "w")?.use { output ->
                source.inputStream().use { it.copyTo(output) }
            } ?: throw IOException("Unable to open export destination.")
            Unit
        }

    override suspend fun createExportStagingPath(fileName: String): String =
        withContext(Dispatchers.IO) {
            exportDir.mkdirs()
            val safeName = fileName.substringAfterLast('/').substringAfterLast('\\')
                .replace(Regex("[^A-Za-z0-9._-]"), "_")
                .take(120)
                .ifBlank { "sona-export" }
            File(exportDir, "${System.nanoTime()}-$safeName").absolutePath
        }

    override suspend fun cleanup(path: String) = withContext(Dispatchers.IO) {
        managedFile(path).delete()
        Unit
    }

    override suspend fun publishText(text: String, destinationUri: String) = withContext(Dispatchers.IO) {
        val uri = requireContentUri(destinationUri)
        context.contentResolver.openOutputStream(uri, "w")?.bufferedWriter(Charsets.UTF_8)?.use {
            it.write(text)
        } ?: throw IOException("Unable to open text export destination.")
    }

    private fun managedFile(path: String): File {
        val file = File(path).canonicalFile
        val allowed = listOf(importDir.canonicalFile, exportDir.canonicalFile)
            .any { root -> isFileWithinRoot(file, root) }
        require(allowed) { "File is outside Sona transfer staging." }
        return file
    }

    private fun requireContentUri(value: String): Uri = Uri.parse(value).also {
        require(it.scheme == ContentResolver.SCHEME_CONTENT) { "SAF URI must use content://." }
    }

    companion object {
        fun create(context: Context): AndroidSafFileTransferAdapter =
            AndroidSafFileTransferAdapter(context.applicationContext)
    }
}
