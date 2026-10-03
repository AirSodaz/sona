package com.sona.android.app.notification

import android.app.PendingIntent
import android.content.Context
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.sona.android.app.R
import com.sona.android.app.feature.settings.ModelDownloadNotificationPort
import com.sona.android.app.feature.settings.ModelDownloadNotificationStage
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDownloadProgress
import com.sona.android.application.recording.LocalAsrDownloadStage

class AndroidModelDownloadNotificationManager(
    context: Context,
) : ModelDownloadNotificationPort {
    private val appContext = context.applicationContext
    private val notificationManager = NotificationManagerCompat.from(appContext)
    private val notificationId = 9527
    override fun update(
        model: LocalAsrCatalogModel,
        progress: LocalAsrDownloadProgress?,
        stage: ModelDownloadNotificationStage,
    ) {
        if (stage == ModelDownloadNotificationStage.CANCELLED) {
            try {
                notificationManager.cancel(notificationId)
            } catch (_: SecurityException) {
            }
            return
        }
        if (!SonaNotificationChannels.areNotificationsEnabled(appContext)) return
        val launchIntent = appContext.packageManager.getLaunchIntentForPackage(appContext.packageName)
        val contentIntent = launchIntent?.let {
            PendingIntent.getActivity(
                appContext,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        val builder = NotificationCompat.Builder(appContext, SonaNotificationChannels.CHANNEL_TASKS)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(model.displayName)
            .setContentIntent(contentIntent)
            .setOnlyAlertOnce(true)

        when (stage) {
            ModelDownloadNotificationStage.STARTING -> {
                builder.setContentText(appContext.getString(R.string.local_model_downloading))
                    .setProgress(100, 0, true)
                    .setOngoing(true)
            }
            ModelDownloadNotificationStage.PROGRESS -> {
                val downloaded = progress?.downloadedBytes ?: 0L
                val total = progress?.totalBytes ?: 0L
                val stageText = when (progress?.stage) {
                    LocalAsrDownloadStage.DOWNLOADING -> appContext.getString(R.string.local_model_downloading)
                    LocalAsrDownloadStage.VERIFYING -> appContext.getString(R.string.local_model_verifying)
                    LocalAsrDownloadStage.INSTALLING -> appContext.getString(R.string.local_model_installing)
                    null -> appContext.getString(R.string.local_model_downloading)
                }
                if (total > 0L) {
                    val percent = ((downloaded * 100) / total).toInt().coerceIn(0, 100)
                    builder.setContentText("$stageText ($percent%)")
                        .setProgress(100, percent, false)
                } else {
                    builder.setContentText(stageText)
                        .setProgress(100, 0, true)
                }
                builder.setOngoing(true)
            }
            ModelDownloadNotificationStage.COMPLETED -> {
                builder.setContentText(appContext.getString(R.string.local_model_downloaded))
                    .setProgress(0, 0, false)
                    .setOngoing(false)
                    .setAutoCancel(true)
            }
            ModelDownloadNotificationStage.FAILED -> {
                builder.setContentText(appContext.getString(R.string.local_model_operation_failed))
                    .setProgress(0, 0, false)
                    .setOngoing(false)
                    .setAutoCancel(true)
            }
            ModelDownloadNotificationStage.CANCELLED -> return
        }
        try {
            notificationManager.notify(notificationId, builder.build())
        } catch (_: SecurityException) {
        }
    }
}
