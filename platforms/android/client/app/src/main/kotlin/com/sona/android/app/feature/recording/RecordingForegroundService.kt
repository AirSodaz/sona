package com.sona.android.app.feature.recording

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.annotation.StringRes
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import com.sona.android.app.BuildConfig
import com.sona.android.app.MainActivity
import com.sona.android.app.R
import com.sona.android.app.SonaApplication
import com.sona.android.application.recording.LiveRecordingState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.StateFlow

class RecordingForegroundService : Service() {
    private val serviceJob = SupervisorJob()
    private val serviceScope = CoroutineScope(serviceJob + Dispatchers.Main.immediate)
    private var foregroundStarted = false
    private var finished = false
    private lateinit var gateway: RecordingForegroundGateway
    private lateinit var sessionState: StateFlow<LiveRecordingState>
    private lateinit var session: RecordingForegroundSession

    override fun onCreate() {
        super.onCreate()
        val container = (application as SonaApplication).container
        gateway = container.recordingGateway
        try {
            val controller = container.createLiveRecording(serviceScope)
            sessionState = controller.state
            gateway.attach(sessionState)
            createNotificationChannel()
            session = RecordingForegroundSession(
                controller = controller,
                scope = serviceScope,
                onPhaseChanged = ::showNotification,
                onFailure = gateway::reportServiceFailure,
                onFinished = ::finishService,
            )
        } catch (_: Exception) {
            gateway.reportServiceFailure()
            finishService()
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (finished || !::session.isInitialized) return START_NOT_STICKY
        try {
            when (intent?.action) {
                ACTION_START -> session.start()
                ACTION_STOP -> session.stop()
                else -> finishService()
            }
        } catch (_: Exception) {
            gateway.reportServiceFailure()
            finishService()
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        val detachImmediately = ::sessionState.isInitialized && !sessionState.value.isActiveSession
        serviceScope.cancel()
        if (detachImmediately) {
            gateway.detach(sessionState)
        }
        super.onDestroy()
    }

    @SuppressLint("InlinedApi", "MissingPermission")
    private fun showNotification(phase: RecordingNotificationPhase) {
        val notification = buildNotification(phase)
        if (!foregroundStarted) {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE,
            )
            foregroundStarted = true
        } else {
            NotificationManagerCompat.from(this).notify(NOTIFICATION_ID, notification)
        }
    }

    private fun buildNotification(phase: RecordingNotificationPhase): Notification {
        val openAppIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val contentIntent = PendingIntent.getActivity(
            this,
            OPEN_APP_REQUEST_CODE,
            openAppIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val stopIntent = PendingIntent.getService(
            this,
            STOP_RECORDING_REQUEST_CODE,
            intent(this, ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(this, NOTIFICATION_CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification_recording)
            .setContentTitle(BuildConfig.APP_NAME)
            .setContentText(getString(phase.messageResource))
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setContentIntent(contentIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .apply {
                if (phase != RecordingNotificationPhase.STOPPING) {
                    addAction(
                        R.drawable.ic_notification_stop,
                        getString(R.string.recording_notification_stop),
                        stopIntent,
                    )
                }
            }
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                NOTIFICATION_CHANNEL_ID,
                getString(R.string.recording_notification_channel),
                NotificationManager.IMPORTANCE_LOW,
            ),
        )
    }

    private fun finishService() {
        if (finished) return
        finished = true
        if (::sessionState.isInitialized) {
            gateway.detach(sessionState)
        }
        if (foregroundStarted) {
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        }
        stopSelf()
    }

    private val RecordingNotificationPhase.messageResource: Int
        @StringRes get() = when (this) {
            RecordingNotificationPhase.PREPARING -> R.string.recording_notification_preparing
            RecordingNotificationPhase.RECORDING -> R.string.recording_notification_active
            RecordingNotificationPhase.AUDIO_ONLY -> R.string.recording_notification_audio_only
            RecordingNotificationPhase.STOPPING -> R.string.recording_notification_stopping
        }

    companion object {
        internal const val ACTION_START = "com.sona.android.action.START_RECORDING"
        internal const val ACTION_STOP = "com.sona.android.action.STOP_RECORDING"
        private const val NOTIFICATION_CHANNEL_ID = "recording"
        private const val NOTIFICATION_ID = 1001
        private const val OPEN_APP_REQUEST_CODE = 1001
        private const val STOP_RECORDING_REQUEST_CODE = 1002

        internal fun intent(context: Context, action: String): Intent =
            Intent(context, RecordingForegroundService::class.java).setAction(action)
    }
}

private val LiveRecordingState.isActiveSession: Boolean
    get() = this is LiveRecordingState.Preparing ||
        this is LiveRecordingState.Recording ||
        this is LiveRecordingState.Stopping
