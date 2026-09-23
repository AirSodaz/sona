package com.sona.android.app.notification

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.app.NotificationManagerCompat
import com.sona.android.app.R

object SonaNotificationChannels {
    const val CHANNEL_RECORDING = "recording"
    const val CHANNEL_AUDIO_IMPORTS = "audio_imports"
    const val CHANNEL_TASKS = "tasks"
    const val PERMISSION_POST_NOTIFICATIONS = "android.permission.POST_NOTIFICATIONS"

    fun createChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(NotificationManager::class.java) ?: return

        val recordingChannel = NotificationChannel(
            CHANNEL_RECORDING,
            context.getString(R.string.recording_notification_channel),
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = context.getString(R.string.recording_notification_channel_description)
            setShowBadge(false)
        }

        val audioImportsChannel = NotificationChannel(
            CHANNEL_AUDIO_IMPORTS,
            context.getString(R.string.audio_import_notification_channel),
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = context.getString(R.string.audio_import_notification_channel_description)
            setShowBadge(false)
        }

        val tasksChannel = NotificationChannel(
            CHANNEL_TASKS,
            context.getString(R.string.tasks_notification_channel),
            NotificationManager.IMPORTANCE_DEFAULT,
        ).apply {
            description = context.getString(R.string.tasks_notification_channel_description)
            setShowBadge(true)
        }

        manager.createNotificationChannels(listOf(recordingChannel, audioImportsChannel, tasksChannel))
    }

    fun areNotificationsEnabled(context: Context): Boolean =
        NotificationManagerCompat.from(context).areNotificationsEnabled()

    fun openNotificationSettings(context: Context) {
        val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
            }
        } else {
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.fromParts("package", context.packageName, null)
            }
        }.apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK
        }
        context.startActivity(intent)
    }
}
