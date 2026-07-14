package com.sona.android.adapters.android.system

import com.sona.android.application.recording.RecordingIdPort
import java.util.UUID

class UuidRecordingIdPort(
    private val uuidSupplier: () -> UUID = UUID::randomUUID,
) : RecordingIdPort {
    override fun nextRecordingId(): String = uuidSupplier().toString()
}
