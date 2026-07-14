package com.sona.android.adapters.android.audio

import com.sona.android.application.recording.AudioInputConfiguration
import com.sona.android.application.recording.AudioInputEvent
import org.junit.Assert.assertEquals
import org.junit.Test

class AudioInputEventMapperTest {
    @Test
    fun `API 23 reports monitoring unavailable exactly once`() {
        val mapper = AudioInputEventMapper(apiLevel = 23, audioSessionId = 41)

        assertEquals(
            listOf(AudioInputEvent.MonitoringUnavailable),
            mapper.onMonitoringStarted(),
        )
        assertEquals(emptyList<AudioInputEvent>(), mapper.onMonitoringStarted())
        assertEquals(
            emptyList<AudioInputEvent>(),
            mapper.onRecordingConfigurationsChanged(listOf(snapshot(sessionId = 41))),
        )
    }

    @Test
    fun `configuration callbacks are filtered by client audio session id`() {
        val mapper = AudioInputEventMapper(apiLevel = 29, audioSessionId = 41)

        val events = mapper.onRecordingConfigurationsChanged(
            listOf(
                snapshot(sessionId = 7, deviceName = "wrong microphone"),
                snapshot(sessionId = 41, deviceName = "built-in microphone"),
            ),
        )

        assertEquals(
            listOf(
                AudioInputEvent.Active,
                AudioInputEvent.ConfigurationChanged(
                    AudioInputConfiguration(
                        deviceName = "built-in microphone",
                        sampleRateHz = 16_000,
                        channelCount = 1,
                        preprocessing = listOf("AcousticEchoCanceler", "NoiseSuppressor"),
                    ),
                ),
            ),
            events,
        )
    }

    @Test
    fun `API 24 through 28 keep silencing unknown and never invent active`() {
        listOf(24, 28).forEach { apiLevel ->
            val mapper = AudioInputEventMapper(apiLevel = apiLevel, audioSessionId = 9)

            val events = mapper.onRecordingConfigurationsChanged(
                listOf(snapshot(sessionId = 9, silenced = false)),
            )

            assertEquals(
                listOf(
                    AudioInputEvent.ConfigurationChanged(
                        AudioInputConfiguration(
                            deviceName = "microphone",
                            sampleRateHz = 16_000,
                            channelCount = 1,
                            preprocessing = listOf("AcousticEchoCanceler", "NoiseSuppressor"),
                        ),
                    ),
                ),
                events,
            )
        }
    }

    @Test
    fun `API 29 emits first values and only subsequent deltas`() {
        val mapper = AudioInputEventMapper(apiLevel = 29, audioSessionId = 9)
        val first = snapshot(sessionId = 9, silenced = false)

        assertEquals(
            listOf(
                AudioInputEvent.Active,
                AudioInputEvent.ConfigurationChanged(
                    AudioInputConfiguration(
                        deviceName = "microphone",
                        sampleRateHz = 16_000,
                        channelCount = 1,
                        preprocessing = listOf("AcousticEchoCanceler", "NoiseSuppressor"),
                    ),
                ),
            ),
            mapper.onRecordingConfigurationsChanged(listOf(first)),
        )
        assertEquals(
            emptyList<AudioInputEvent>(),
            mapper.onRecordingConfigurationsChanged(listOf(first)),
        )
        assertEquals(
            listOf(AudioInputEvent.Silenced),
            mapper.onRecordingConfigurationsChanged(listOf(first.copy(silenced = true))),
        )
        assertEquals(
            listOf(
                AudioInputEvent.ConfigurationChanged(
                    AudioInputConfiguration(
                        deviceName = "USB microphone",
                        sampleRateHz = 48_000,
                        channelCount = 2,
                        preprocessing = listOf("NoiseSuppressor"),
                    ),
                ),
            ),
            mapper.onRecordingConfigurationsChanged(
                listOf(
                    first.copy(
                        silenced = true,
                        deviceName = "USB microphone",
                        sampleRateHz = 48_000,
                        channelCount = 2,
                        preprocessing = listOf("NoiseSuppressor"),
                    ),
                ),
            ),
        )
    }

    @Test
    fun `missing target configuration does not change remembered state`() {
        val mapper = AudioInputEventMapper(apiLevel = 29, audioSessionId = 9)
        val target = snapshot(sessionId = 9)
        mapper.onRecordingConfigurationsChanged(listOf(target))

        assertEquals(
            emptyList<AudioInputEvent>(),
            mapper.onRecordingConfigurationsChanged(listOf(snapshot(sessionId = 10))),
        )
        assertEquals(
            emptyList<AudioInputEvent>(),
            mapper.onRecordingConfigurationsChanged(listOf(target)),
        )
    }

    private fun snapshot(
        sessionId: Int,
        silenced: Boolean? = false,
        deviceName: String? = "microphone",
        sampleRateHz: Int? = 16_000,
        channelCount: Int? = 1,
        preprocessing: List<String> = listOf("NoiseSuppressor", "AcousticEchoCanceler"),
    ) = AudioRecordingSnapshot(
        clientAudioSessionId = sessionId,
        silenced = silenced,
        deviceName = deviceName,
        sampleRateHz = sampleRateHz,
        channelCount = channelCount,
        preprocessing = preprocessing,
    )
}
