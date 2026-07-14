package com.sona.android.adapters.android.audio

import com.sona.android.application.recording.AudioInputEvent

interface AudioCaptureBackend : AutoCloseable {
    val bufferSizeBytes: Int

    fun start()

    fun read(buffer: ByteArray): Int

    fun stop()
}

fun interface AudioInputEventListener {
    fun onInputEvent(event: AudioInputEvent)
}

interface AudioInputMonitoringBackend {
    fun setInputEventListener(listener: AudioInputEventListener?)
}
