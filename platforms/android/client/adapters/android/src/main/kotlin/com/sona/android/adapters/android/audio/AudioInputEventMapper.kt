package com.sona.android.adapters.android.audio

import com.sona.android.application.recording.AudioInputConfiguration
import com.sona.android.application.recording.AudioInputEvent

internal data class AudioRecordingSnapshot(
    val clientAudioSessionId: Int,
    val silenced: Boolean?,
    val deviceName: String?,
    val sampleRateHz: Int?,
    val channelCount: Int?,
    val preprocessing: List<String>,
)

internal class AudioInputEventMapper(
    private val apiLevel: Int,
    private val audioSessionId: Int,
) {
    private var monitoringUnavailableEmitted = false
    private var lastSilenced: Boolean? = null
    private var hasSilencedValue = false
    private var lastConfiguration: AudioInputConfiguration? = null

    fun onMonitoringStarted(): List<AudioInputEvent> {
        if (apiLevel != API_WITHOUT_MONITORING || monitoringUnavailableEmitted) {
            return emptyList()
        }
        monitoringUnavailableEmitted = true
        return listOf(AudioInputEvent.MonitoringUnavailable)
    }

    fun onRecordingConfigurationsChanged(
        configurations: List<AudioRecordingSnapshot>,
    ): List<AudioInputEvent> {
        if (apiLevel < API_WITH_MONITORING) {
            return emptyList()
        }
        val snapshot = configurations.firstOrNull {
            it.clientAudioSessionId == audioSessionId
        } ?: return emptyList()

        val events = mutableListOf<AudioInputEvent>()
        if (apiLevel >= API_WITH_SILENCING && snapshot.silenced != null) {
            if (!hasSilencedValue || lastSilenced != snapshot.silenced) {
                events += if (snapshot.silenced) {
                    AudioInputEvent.Silenced
                } else {
                    AudioInputEvent.Active
                }
                hasSilencedValue = true
                lastSilenced = snapshot.silenced
            }
        }

        val configuration = AudioInputConfiguration(
            deviceName = snapshot.deviceName,
            sampleRateHz = snapshot.sampleRateHz,
            channelCount = snapshot.channelCount,
            preprocessing = snapshot.preprocessing.sorted(),
        )
        if (lastConfiguration != configuration) {
            events += AudioInputEvent.ConfigurationChanged(configuration)
            lastConfiguration = configuration
        }
        return events
    }

    private companion object {
        const val API_WITHOUT_MONITORING = 23
        const val API_WITH_MONITORING = 24
        const val API_WITH_SILENCING = 29
    }
}
