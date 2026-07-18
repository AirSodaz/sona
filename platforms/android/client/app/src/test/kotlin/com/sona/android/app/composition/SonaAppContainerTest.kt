package com.sona.android.app.composition

import com.sona.android.application.sync.SyncSecretStorePort
import org.junit.Assert.assertEquals
import org.junit.Test

class SonaAppContainerTest {
    @Test
    fun `container exposes the Android sync secret store through its application port`() {
        val getter = SonaAppContainer::class.java.methods.single { method ->
            method.name == "getSyncSecrets"
        }

        assertEquals(SyncSecretStorePort::class.java, getter.returnType)
    }
}
