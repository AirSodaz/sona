package com.sona.android.application.sync

import kotlin.coroutines.Continuation
import org.junit.Assert.assertEquals
import org.junit.Test

class SyncSecretStorePortTest {
    @Test
    fun `secret store exposes asynchronous get set and delete operations`() {
        val operations = SyncSecretStorePort::class.java.declaredMethods.associateBy { it.name }

        assertEquals(
            listOf(String::class.java, Continuation::class.java),
            operations.getValue("get").parameterTypes.toList(),
        )
        assertEquals(
            listOf(String::class.java, ByteArray::class.java, Continuation::class.java),
            operations.getValue("set").parameterTypes.toList(),
        )
        assertEquals(
            listOf(String::class.java, Continuation::class.java),
            operations.getValue("delete").parameterTypes.toList(),
        )
    }
}
