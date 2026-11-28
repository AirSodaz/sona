package com.sona.android.adapters.android.settings

import android.content.Context
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.AppLogPort
import java.io.File
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
    private val primaryFile: File get() = File(logsDir, PRIMARY_LOG_FILE)
    private val rotatedFile: File get() = File(logsDir, ROTATED_LOG_FILE)

    val logDirectoryPath: String get() = logsDir.absolutePath

    override fun currentLogLevel(): AppLogLevel = currentLevel

    override fun applyLogLevel(level: AppLogLevel) {
        currentLevel = level
        onLevelChanged(level)
    }

    fun append(level: String, target: String, message: String) {
        val parsed = AppLogLevel.fromWireValue(level, AppLogLevel.INFO)
        if (!currentLevel.allows(parsed)) return

        val timestamp = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US).format(Date())
        val line = "[$timestamp] [${parsed.wireValue.uppercase()}] [$target] ${message.trimEnd()}\n"
        synchronized(writeLock) {
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
        synchronized(writeLock) {
            val builder = StringBuilder()
            if (rotatedFile.isFile) {
                builder.append(rotatedFile.readText(Charsets.UTF_8))
            }
            if (primaryFile.isFile) {
                builder.append(primaryFile.readText(Charsets.UTF_8))
            }
            val full = builder.toString()
            if (full.length <= maxBytes) {
                full
            } else {
                full.takeLast(maxBytes)
            }
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
