package com.sona.android.adapters.android.data

import java.io.File

internal fun isFileWithinRoot(file: File, root: File): Boolean {
    val rootPath = root.canonicalPath.trimEnd(File.separatorChar) + File.separator
    return file.canonicalPath.startsWith(rootPath)
}
