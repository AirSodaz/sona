package com.sona.android.application.recording

import org.junit.Assert.assertFalse
import org.junit.Test

class RecordingPortsContractTest {
    @Test
    fun `streaming credentials never expose their api key in string form`() {
        val credential = StreamingCredential(apiKey = "secret")

        assertFalse(credential.toString().contains("secret"))
    }
}
