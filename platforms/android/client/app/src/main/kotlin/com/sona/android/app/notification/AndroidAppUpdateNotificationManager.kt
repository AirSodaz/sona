package com.sona.android.app.notification

import android.app.PendingIntent
import android.content.Context
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.sona.android.app.R
import com.sona.android.app.feature.settings.AppUpdateNotificationPort
import com.sona.android.application.settings.AppUpdateDownloadProgress
import com.sona.android.application.settings.AppUpdateDownloadStage

class AndroidAppUpdateNotificationManager(
    context: Context,
) : AppUpdateNotificationPort {
    private val appContext = context.applicationContext
    private val notificationManager = NotificationManagerCompat.from(appContext)
    private val notificationId = 9528

    override fun updateProgress(versionName: String, progress: AppUpdateDownloadProgress) {
        if (!SonaNotificationChannels.areNotificationsEnabled(appContext)) return
        val builder = baseBuilder(versionName)
        when (progress.stage) {
            AppUpdateDownloadStage.DOWNLOADING -> {
                val total = progress.totalBytes ?: 0L
                if (total > 0L) {
                    val percent = ((progress.downloadedBytes * 100) / total).toInt().coerceIn(0, 100)
                    builder.setContentText(
                        appContext.getString(R.string.about_update_downloading_percent, percent),
                    )
                        .setProgress(100, percent, false)
                } else {
                    builder.setContentText(appContext.getString(R.string.about_update_downloading))
                        .setProgress(100, 0, true)
                }
                builder.setOngoing(true)
            }
            AppUpdateDownloadStage.VERIFYING -> {
                builder.setContentText(appContext.getString(R.string.about_update_verifying))
                    .setProgress(100, 0, true)
                    .setOngoing(true)
            }
            AppUpdateDownloadStage.READY -> {
                showReadyToInstall(versionName)
                return
            }
        }
        notifySafely(builder)
    }

    override fun showReadyToInstall(versionName: String) {
        if (!SonaNotificationChannels.areNotificationsEnabled(appContext)) return
        val builder = baseBuilder(versionName)
            .setContentText(appContext.getString(R.string.about_update_ready_to_install, versionName))
            .setProgress(0, 0, false)
            .setOngoing(false)
            .setAutoCancel(true)
        notifySafely(builder)
    }

    override fun showFailed(versionName: String) {
        if (!SonaNotificationChannels.areNotificationsEnabled(appContext)) return
        val builder = baseBuilder(versionName)
            .setContentText(appContext.getString(R.string.about_update_download_failed))
            .setProgress(0, 0, false)
            .setOngoing(false)
            .setAutoCancel(true)
        notifySafely(builder)
    }

    override fun cancel() {
        try {
            notificationManager.cancel(notificationId)
        } catch (_: SecurityException) {
        }
    }

    private fun baseBuilder(versionName: String): NotificationCompat.Builder {
        val launchIntent = appContext.packageManager.getLaunchIntentForPackage(appContext.packageName)
        val contentIntent = launchIntent?.let {
            PendingIntent.getActivity(
                appContext,
                notificationId,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        return NotificationCompat.Builder(appContext, SonaNotificationChannels.CHANNEL_TASKS)
            .setSmallIcon(R.drawable.ic_notification_recording)
            .setContentTitle(appContext.getString(R.string.about_update_notification_title, versionName))
            .setContentIntent(contentIntent)
            .setOnlyAlertOnce(true)
    }

    private fun notifySafely(builder: NotificationCompat.Builder) {
        try {
            notificationManager.notify(notificationId, builder.build())
        } catch (_: SecurityException) {
        }
    }
}
