package com.sona.android.adapters.android.sync

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidSyncSecretStoreTest {
    @Test
    fun `logical key scopes both persisted records and cipher AAD`() = runBlocking {
        val records = FakeSyncSecretRecordStore()
        val cipher = FakeSyncSecretCipher()
        val store = AndroidSyncSecretStore(records, cipher)
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"

        store.set(passwordKey, "password".encodeToByteArray())
        store.set(vaultKey, byteArrayOf(1, 2, 3))

        assertArrayEquals("password".encodeToByteArray(), store.get(passwordKey))
        assertArrayEquals(byteArrayOf(1, 2, 3), store.get(vaultKey))
        assertTrue(records.contains(passwordKey))
        assertTrue(records.contains(vaultKey))
        assertTrue(cipher.encryptKeys.containsAll(listOf(passwordKey, vaultKey)))
        assertTrue(cipher.decryptKeys.containsAll(listOf(passwordKey, vaultKey)))
    }

    @Test
    fun `delete removes only the requested logical key`() = runBlocking {
        val records = FakeSyncSecretRecordStore()
        val store = AndroidSyncSecretStore(records, FakeSyncSecretCipher())
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"
        store.set(passwordKey, byteArrayOf(1))
        store.set(vaultKey, byteArrayOf(2))

        store.delete(passwordKey)

        assertFalse(records.contains(passwordKey))
        assertTrue(records.contains(vaultKey))
        assertNull(store.get(passwordKey))
        assertArrayEquals(byteArrayOf(2), store.get(vaultKey))
    }

    @Test
    fun `permanently unreadable ciphertext is deleted without deleting another logical key`() = runBlocking {
        val records = FakeSyncSecretRecordStore()
        val cipher = FakeSyncSecretCipher()
        val store = AndroidSyncSecretStore(records, cipher)
        val invalidKey = "webdav-password:vault-a"
        val otherKey = "vault-key:vault-a"
        store.set(invalidKey, byteArrayOf(1))
        store.set(otherKey, byteArrayOf(2))
        cipher.decryptFailures[invalidKey] = SyncSecretCipherException(
            SyncSecretCipherFailureKind.PERMANENT_RECORD,
        )

        assertNull(store.get(invalidKey))

        assertFalse(records.contains(invalidKey))
        assertTrue(records.contains(otherKey))
        assertArrayEquals(byteArrayOf(2), store.get(otherKey))
    }

    @Test
    fun `permanently invalidated shared key clears records and rotates before the next write`() = runBlocking {
        val records = FakeSyncSecretRecordStore()
        val cipher = FakeSyncSecretCipher()
        val store = AndroidSyncSecretStore(records, cipher)
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"
        store.set(passwordKey, byteArrayOf(1))
        store.set(vaultKey, byteArrayOf(2))
        cipher.decryptFailures[passwordKey] = SyncSecretCipherException(
            SyncSecretCipherFailureKind.PERMANENT_KEY,
        )

        assertNull(store.get(passwordKey))

        assertEquals(1, cipher.resetCount)
        assertFalse(records.contains(passwordKey))
        assertFalse(records.contains(vaultKey))
        store.set(vaultKey, byteArrayOf(9))
        assertArrayEquals(byteArrayOf(9), store.get(vaultKey))
    }

    @Test
    fun `temporary decrypt failure propagates and preserves the encrypted record`() = runBlocking {
        val records = FakeSyncSecretRecordStore()
        val cipher = FakeSyncSecretCipher()
        val store = AndroidSyncSecretStore(records, cipher)
        val key = "vault-key:vault-a"
        store.set(key, byteArrayOf(1, 2, 3))
        val expected = SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
        cipher.decryptFailures[key] = expected

        val actual = captureCipherFailure { store.get(key) }

        assertSame(expected, actual)
        assertTrue(records.contains(key))
    }

    @Test
    fun `temporary encrypt failure propagates and preserves the previous encrypted record`() = runBlocking {
        val records = FakeSyncSecretRecordStore()
        val cipher = FakeSyncSecretCipher()
        val store = AndroidSyncSecretStore(records, cipher)
        val key = "vault-key:vault-a"
        store.set(key, byteArrayOf(1, 2, 3))
        val previous = records.get(key)
        val expected = SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
        cipher.encryptFailures[key] = expected

        val actual = captureCipherFailure { store.set(key, byteArrayOf(4, 5, 6)) }

        assertSame(expected, actual)
        assertTrue(previous === records.get(key))
        cipher.encryptFailures.remove(key)
        assertArrayEquals(byteArrayOf(1, 2, 3), store.get(key))
    }

    private suspend fun captureCipherFailure(
        block: suspend () -> Unit,
    ): SyncSecretCipherException = try {
        block()
        throw AssertionError("Expected SyncSecretCipherException")
    } catch (error: SyncSecretCipherException) {
        error
    }
}

private class FakeSyncSecretRecordStore : SyncSecretRecordStore {
    private val records = mutableMapOf<String, SyncSecretEnvelope>()

    override suspend fun get(logicalKey: String): SyncSecretEnvelope? = records[logicalKey]

    override suspend fun set(logicalKey: String, envelope: SyncSecretEnvelope) {
        records[logicalKey] = envelope
    }

    override suspend fun delete(logicalKey: String) {
        records.remove(logicalKey)
    }

    override suspend fun clear() {
        records.clear()
    }

    fun contains(logicalKey: String): Boolean = records.containsKey(logicalKey)
}

private class FakeSyncSecretCipher : SyncSecretCipher {
    val encryptKeys = mutableListOf<String>()
    val decryptKeys = mutableListOf<String>()
    val encryptFailures = mutableMapOf<String, SyncSecretCipherException>()
    val decryptFailures = mutableMapOf<String, SyncSecretCipherException>()
    var resetCount = 0

    override fun encrypt(logicalKey: String, plaintext: ByteArray): SyncSecretEnvelope {
        encryptKeys += logicalKey
        encryptFailures[logicalKey]?.let { throw it }
        return SyncSecretEnvelope(
            iv = ByteArray(12) { (logicalKey.hashCode() + it).toByte() },
            ciphertext = ByteArray(16) + plaintext,
        )
    }

    override fun decrypt(logicalKey: String, envelope: SyncSecretEnvelope): ByteArray {
        decryptKeys += logicalKey
        decryptFailures[logicalKey]?.let { throw it }
        return envelope.ciphertext.copyOfRange(16, envelope.ciphertext.size)
    }

    override fun resetAfterKeyInvalidation() {
        resetCount += 1
        encryptFailures.entries.removeAll { it.value.kind == SyncSecretCipherFailureKind.PERMANENT_KEY }
        decryptFailures.entries.removeAll { it.value.kind == SyncSecretCipherFailureKind.PERMANENT_KEY }
    }
}
