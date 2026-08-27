package com.sona.android.application.recording

import org.junit.Assert.assertFalse
import org.junit.Test

class OnlineBatchPortsContractTest {
    @Test
    fun `online batch credentials never expose their secret in string form`() {
        val credential = OnlineBatchCredential("temporary-secret")

        assertFalse(credential.toString().contains("temporary-secret"))
    }
}
