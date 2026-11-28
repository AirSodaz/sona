package com.sona.android.adapters.android.settings

import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import androidx.core.net.toUri
import com.sona.android.application.settings.ApkInstallerPort
import com.sona.android.application.settings.AppUpdateErrorCategory
import com.sona.android.application.settings.AppUpdateException
import java.io.File

class AndroidApkInstaller(
    context: Context,
) : ApkInstallerPort {
    private val appContext = context.applicationContext

    override fun supportedAbis(): List<String> = Build.SUPPORTED_ABIS.toList()

    override fun canRequestPackageInstalls(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            appContext.packageManager.canRequestPackageInstalls()
        } else {
            true
        }

    override fun installApk(apkPath: String) {
        val apkFile = File(apkPath)
        if (!apkFile.isFile) {
            throw AppUpdateException(
                AppUpdateErrorCategory.INSTALL_FAILED,
                "Update APK file does not exist.",
            )
        }
        val uri = FileProvider.getUriForFile(
            appContext,
            "${appContext.packageName}.fileprovider",
            apkFile,
        )
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        try {
            appContext.startActivity(intent)
        } catch (error: Throwable) {
            throw AppUpdateException(
                AppUpdateErrorCategory.INSTALL_FAILED,
                error.message ?: "Unable to launch package installer.",
                error,
            )
        }
    }

    override fun openInstallPermissionSettings() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val intent = Intent(
            Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
            "package:${appContext.packageName}".toUri(),
        ).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        appContext.startActivity(intent)
    }
}
