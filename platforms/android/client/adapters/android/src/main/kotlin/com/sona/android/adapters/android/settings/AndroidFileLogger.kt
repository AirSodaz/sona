package com.sona.android.adapters.android.settings

import android.content.Context
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.AppLogPort
import java.io.File
import java.io.RandomAccessFile
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class AndroidFileLogger internal constructor(
    private val logsDir: File,
    initialLevel: AppLogLevel = AppLogLevel.INFO,
    private val maxFileBytes: Long = DEFAULT_MAX_FILE_BYTES,
    private val onLevelChanged: (AppLogLevel) -> Unit = {},
) : AppLogPort {
    constructor(
        context: Context,
        initialLevel: AppLogLevel = AppLogLevel.INFO,
        onLevelChanged: (AppLogLevel) -> Unit = {},
    ) : this(
        logsDir = File(context.applicationContext.filesDir, "logs"),
        initialLevel = initialLevel,
        onLevelChanged = onLevelChanged,
    )

    @Volatile
    private var currentLevel: AppLogLevel = initialLevel
    private val writeLock = Any()
    private val timestampFormat = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)
    private val primaryFile: File get() = File(logsDir, PRIMARY_LOG_FILE)
    private val rotatedFile: File get() = File(logsDir, ROTATED_LOG_FILE)

    val logDirectoryPath: String get() = logsDir.absolutePath

    override fun currentLogLevel(): AppLogLevel = currentLevel

    override fun applyLogLevel(level: AppLogLevel) {
        if (currentLevel == level) return
        currentLevel = level
        onLevelChanged(level)
    }

    fun append(level: String, target: String, message: String) {
        val parsed = AppLogLevel.fromWireValue(level, AppLogLevel.INFO)
        if (!currentLevel.allows(parsed)) return

        synchronized(writeLock) {
            val timestamp = timestampFormat.format(Date())
            val line = "[$timestamp] [${parsed.wireValue.uppercase()}] [$target] ${message.trimEnd()}\n"
            runCatching {
                logsDir.mkdirs()
                if (primaryFile.exists() && primaryFile.length() >= maxFileBytes) {
                    if (rotatedFile.exists()) {
                        rotatedFile.delete()
                    }
                    primaryFile.renameTo(rotatedFile)
                }
                primaryFile.appendText(line, Charsets.UTF_8)
            }
        }
    }

    override suspend fun logSizeBytes(): Long = withContext(Dispatchers.IO) {
        synchronized(writeLock) {
            (if (primaryFile.isFile) primaryFile.length() else 0L) +
                (if (rotatedFile.isFile) rotatedFile.length() else 0L)
        }
    }

    override suspend fun readRecentLogs(maxBytes: Int): String = withContext(Dispatchers.IO) {
        if (maxBytes <= 0) return@withContext ""
        synchronized(writeLock) {
            val primaryTail = readTailBytes(primaryFile, maxBytes)
            val remaining = maxBytes - primaryTail.size
            val rotatedTail = if (remaining > 0) readTailBytes(rotatedFile, remaining) else ByteArray(0)
            val combined = if (rotatedTail.isEmpty()) {
                primaryTail
            } else {
                rotatedTail + primaryTail
            }
            if (combined.isEmpty()) return@withContext ""
            var startOffset = 0
            val wasTruncated = (rotatedFile.isFile && rotatedFile.length() > rotatedTail.size) ||
                (primaryFile.isFile && primaryFile.length() > primaryTail.size)
            if (wasTruncated) {
                while (startOffset < combined.size && (combined[startOffset].toInt() and 0xC0) == 0x80) {
                    startOffset += 1
                }
            }
            String(combined, startOffset, combined.size - startOffset, Charsets.UTF_8)
        }
    }

    private fun readTailBytes(file: File, maxBytes: Int): ByteArray {
        if (!file.isFile || maxBytes <= 0) return ByteArray(0)
        val length = file.length()
        if (length <= 0L) return ByteArray(0)
        val toRead = minOf(length, maxBytes.toLong()).toInt()
        return RandomAccessFile(file, "r").use { raf ->
            raf.seek(length - toRead)
            val buffer = ByteArray(toRead)
            raf.readFully(buffer)
            buffer
        }
    }

    override suspend fun clearLogs() {
        withContext(Dispatchers.IO) {
            synchronized(writeLock) {
                primaryFile.delete()
                rotatedFile.delete()
            }
        }
    }

    companion object {
        private const val PRIMARY_LOG_FILE = "sona-android.log"
        private const val ROTATED_LOG_FILE = "sona-android.1.log"
        internal const val DEFAULT_MAX_FILE_BYTES = 1024L * 1024L
    }
}
