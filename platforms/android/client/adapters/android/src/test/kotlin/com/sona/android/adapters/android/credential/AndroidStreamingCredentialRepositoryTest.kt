package com.sona.android.adapters.android.credential

import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.StreamingCredential
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidStreamingCredentialRepositoryTest {
    @Test
    fun `status projects a complete record without decrypting`() = runBlocking {
        val store = FakeCredentialStore(supportedRecord("secret"))
        val cipher = FakeCredentialCipher().apply { decryptFailure = AssertionError("must not decrypt") }
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        assertEquals(CredentialStatus.CONFIGURED, repository.status.first())
        assertEquals(0, cipher.decryptCalls)
    }

    @Test
    fun `load returns a valid credential and clears decrypted bytes`() = runBlocking {
        val cipher = FakeCredentialCipher()
        val envelope = cipher.encrypt("valid-key".encodeToByteArray())
        val repository = AndroidStreamingCredentialRepository(
            store = FakeCredentialStore(envelope.toRecord()),
            cipher = cipher,
        )

        val loaded = repository.loadForStart()

        assertEquals(StreamingCredential("valid-key"), loaded)
        assertTrue(cipher.lastReturnedPlaintext!!.all { it == 0.toByte() })
    }

    @Test
    fun `ordinary overwrite reuses key and writes a fresh iv`() = runBlocking {
        val store = FakeCredentialStore()
        val cipher = FakeCredentialCipher()
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        repository.save(StreamingCredential("same-key"))
        val first = store.current
        repository.save(StreamingCredential("same-key"))
        val second = store.current

        assertFalse(first.ivBase64 == second.ivBase64)
        assertEquals(1, cipher.deleteCalls)
        assertEquals(2, cipher.encryptCalls)
    }

    @Test
    fun `blank and oversized credentials are rejected with a fixed public error`() = runBlocking {
        val repository = AndroidStreamingCredentialRepository(FakeCredentialStore(), FakeCredentialCipher())

        listOf("", " \t\n", "a".repeat(16_385)).forEach { apiKey ->
            val error = captureCredentialError { repository.save(StreamingCredential(apiKey)) }
            assertEquals(CredentialErrorCode.INVALID_CREDENTIAL, error.code)
            assertEquals("Streaming credential is invalid.", error.message)
            assertNull(error.cause)
        }
    }

    @Test
    fun `maximum utf8 credential size is accepted and source bytes are cleared`() = runBlocking {
        val cipher = FakeCredentialCipher()
        val repository = AndroidStreamingCredentialRepository(FakeCredentialStore(), cipher)

        repository.save(StreamingCredential("a".repeat(16_384)))

        assertEquals(1, cipher.encryptCalls)
        assertTrue(cipher.lastReceivedPlaintext!!.all { it == 0.toByte() })
    }

    @Test
    fun `clear is idempotent and always clears store before deleting alias`() = runBlocking {
        val operations = mutableListOf<String>()
        val store = FakeCredentialStore(supportedRecord("secret"), operations)
        val cipher = FakeCredentialCipher(operations)
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        repository.clear()
        repository.clear()

        assertEquals(listOf("store.clear", "cipher.delete", "store.clear", "cipher.delete"), operations)
        assertEquals(CredentialRecord(), store.current)
    }

    @Test
    fun `unknown versions are preserved and block load and save`() = runBlocking {
        val unknown = CredentialRecord(formatVersion = 7)
        val store = FakeCredentialStore(unknown)
        val cipher = FakeCredentialCipher()
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        assertEquals(CredentialStatus.CONFIGURED, repository.status.first())
        val loadError = captureCredentialError { repository.loadForStart() }
        val saveError = captureCredentialError { repository.save(StreamingCredential("replacement")) }

        assertEquals(CredentialErrorCode.UNSUPPORTED_FORMAT, loadError.code)
        assertEquals(CredentialErrorCode.UNSUPPORTED_FORMAT, saveError.code)
        assertEquals("Streaming credential format is unsupported.", loadError.message)
        assertEquals(unknown, store.current)
        assertEquals(0, store.clearCalls)
        assertEquals(0, cipher.encryptCalls)
        assertEquals(0, cipher.deleteCalls)
    }

    @Test
    fun `non-positive version is cleared as malformed`() = runBlocking {
        val operations = mutableListOf<String>()
        val invalid = CredentialRecord(0, "corrupt-iv", "corrupt-ciphertext")
        val store = FakeCredentialStore(invalid, operations)
        val cipher = FakeCredentialCipher(operations)
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        assertNull(repository.loadForStart())

        assertEquals(listOf("store.read", "store.clear", "cipher.delete"), operations)
        assertEquals(CredentialRecord(), store.current)
    }

    @Test
    fun `malformed record is cleared and alias rotated before save`() = runBlocking {
        val operations = mutableListOf<String>()
        val malformed = CredentialRecord(formatVersion = 1, ivBase64 = "partial")
        val store = FakeCredentialStore(malformed, operations)
        val cipher = FakeCredentialCipher(operations)
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        repository.save(StreamingCredential("replacement"))

        assertEquals(
            listOf("store.read", "store.clear", "cipher.delete", "cipher.encrypt", "store.write"),
            operations,
        )
        assertTrue(CredentialEnvelope.inspect(store.current) is CredentialEnvelopeState.Supported)
    }

    @Test
    fun `permanent decrypt failure clears envelope then rotates alias and returns null`() = runBlocking {
        val operations = mutableListOf<String>()
        val store = FakeCredentialStore(supportedRecord("secret"), operations)
        val cipher = FakeCredentialCipher(operations).apply {
            decryptFailure = CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
        }
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        val loaded = repository.loadForStart()

        assertNull(loaded)
        assertEquals(listOf("store.read", "cipher.decrypt", "store.clear", "cipher.delete"), operations)
        assertEquals(CredentialRecord(), store.current)
    }

    @Test
    fun `save after permanent invalidation removes any orphan before recreating`() = runBlocking {
        val operations = mutableListOf<String>()
        val store = FakeCredentialStore(supportedRecord("secret"), operations)
        val cipher = FakeCredentialCipher(operations).apply {
            decryptFailure = CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
        }
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        assertNull(repository.loadForStart())
        repository.save(StreamingCredential("replacement"))
        cipher.decryptFailure = null

        assertEquals(2, cipher.deleteCalls)
        assertTrue(operations.indexOfLast { it == "cipher.delete" } < operations.indexOf("cipher.encrypt"))
        assertEquals(StreamingCredential("replacement"), repository.loadForStart())
    }

    @Test
    fun `temporary decrypt failure preserves envelope and alias`() = runBlocking {
        val original = supportedRecord("secret")
        val store = FakeCredentialStore(original)
        val cipher = FakeCredentialCipher().apply {
            decryptFailure = CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
        }
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        val error = captureCredentialError { repository.loadForStart() }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals(original, store.current)
        assertEquals(0, store.clearCalls)
        assertEquals(0, cipher.deleteCalls)
    }

    @Test
    fun `temporary encrypt failure preserves an existing envelope and alias`() = runBlocking {
        val original = supportedRecord("secret")
        val store = FakeCredentialStore(original)
        val cipher = FakeCredentialCipher().apply {
            encryptFailure = CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
        }
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        val error = captureCredentialError { repository.save(StreamingCredential("replacement")) }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals(original, store.current)
        assertEquals(0, store.clearCalls)
        assertEquals(0, cipher.deleteCalls)
    }

    @Test
    fun `permanent encrypt failure clears envelope before rotating alias`() = runBlocking {
        val operations = mutableListOf<String>()
        val store = FakeCredentialStore(supportedRecord("secret"), operations)
        val cipher = FakeCredentialCipher(operations).apply {
            encryptFailure = CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
        }
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        val error = captureCredentialError { repository.save(StreamingCredential("replacement")) }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals(
            listOf("store.read", "cipher.encrypt", "store.clear", "cipher.delete"),
            operations,
        )
        assertEquals(CredentialRecord(), store.current)
    }

    @Test
    fun `storage failure is redacted and preserves the record`() = runBlocking {
        val sentinel = "sentinel-secret /private/path alias-value iv-value ciphertext-value"
        val original = supportedRecord("secret")
        val store = FakeCredentialStore(original).apply { readFailure = IllegalStateException(sentinel) }
        val repository = AndroidStreamingCredentialRepository(store, FakeCredentialCipher())

        val error = captureCredentialError { repository.loadForStart() }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals("Streaming credential storage is unavailable.", error.message)
        assertFalse(error.toString().contains(sentinel))
        assertFalse(error.stackTraceToString().contains(sentinel))
        assertNull(error.cause)
        assertEquals(original, store.current)
    }

    @Test
    fun `write failure preserves the previous envelope and alias`() = runBlocking {
        val original = supportedRecord("secret")
        val store = FakeCredentialStore(original).apply {
            writeFailure = IllegalStateException("write-path-and-secret")
        }
        val cipher = FakeCredentialCipher()
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        val error = captureCredentialError { repository.save(StreamingCredential("replacement")) }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals(original, store.current)
        assertEquals(0, store.clearCalls)
        assertEquals(0, cipher.deleteCalls)
    }

    @Test
    fun `clear failure preserves the envelope and does not delete the alias`() = runBlocking {
        val original = supportedRecord("secret")
        val store = FakeCredentialStore(original).apply {
            clearFailure = IllegalStateException("clear-path-and-secret")
        }
        val cipher = FakeCredentialCipher()
        val repository = AndroidStreamingCredentialRepository(store, cipher)

        val error = captureCredentialError { repository.clear() }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals(original, store.current)
        assertEquals(0, cipher.deleteCalls)
    }

    @Test
    fun `cancellation is rethrown by save load clear and status without mutation`() = runBlocking {
        val original = supportedRecord("secret")

        val saveCancellation = CancellationException("save-cancelled")
        val saveStore = FakeCredentialStore(original).apply { writeFailure = saveCancellation }
        val saveCipher = FakeCredentialCipher()
        val saveRepository = AndroidStreamingCredentialRepository(saveStore, saveCipher)
        assertTrue(
            captureCancellation { saveRepository.save(StreamingCredential("replacement")) } ===
                saveCancellation,
        )
        assertEquals(original, saveStore.current)
        assertEquals(0, saveCipher.deleteCalls)

        val loadCancellation = CancellationException("load-cancelled")
        val loadStore = FakeCredentialStore(original).apply { readFailure = loadCancellation }
        val loadCipher = FakeCredentialCipher()
        val loadRepository = AndroidStreamingCredentialRepository(loadStore, loadCipher)
        assertTrue(captureCancellation { loadRepository.loadForStart() } === loadCancellation)
        assertEquals(original, loadStore.current)
        assertEquals(0, loadCipher.deleteCalls)

        val clearCancellation = CancellationException("clear-cancelled")
        val clearStore = FakeCredentialStore(original).apply { clearFailure = clearCancellation }
        val clearCipher = FakeCredentialCipher()
        val clearRepository = AndroidStreamingCredentialRepository(clearStore, clearCipher)
        assertTrue(captureCancellation { clearRepository.clear() } === clearCancellation)
        assertEquals(original, clearStore.current)
        assertEquals(0, clearCipher.deleteCalls)

        val statusCancellation = CancellationException("status-cancelled")
        val statusStore = FakeCredentialStore(original).apply { recordsFailure = statusCancellation }
        val statusRepository = AndroidStreamingCredentialRepository(statusStore, FakeCredentialCipher())
        assertTrue(captureCancellation { statusRepository.status.first() } === statusCancellation)
        assertEquals(original, statusStore.current)
    }

    @Test
    fun `concurrent save clear and load are serialized deterministically`() = runBlocking {
        val firstReadEntered = CompletableDeferred<Unit>()
        val releaseFirstRead = CompletableDeferred<Unit>()
        val store = FakeCredentialStore().apply {
            blockFirstRead = {
                firstReadEntered.complete(Unit)
                releaseFirstRead.await()
            }
            operationDelayMillis = 1
        }
        val repository = AndroidStreamingCredentialRepository(store, FakeCredentialCipher())

        val save = launch(start = CoroutineStart.UNDISPATCHED) {
            repository.save(StreamingCredential("queued"))
        }
        firstReadEntered.await()
        val clear = launch(start = CoroutineStart.UNDISPATCHED) { repository.clear() }
        val load = async(start = CoroutineStart.UNDISPATCHED) { repository.loadForStart() }
        releaseFirstRead.complete(Unit)

        save.join()
        clear.join()
        assertNull(load.await())
        assertEquals(1, store.maxConcurrentOperations.get())
        assertEquals(CredentialRecord(), store.current)
    }

    @Test
    fun `repository exposes only capability port methods to each consumer`() {
        val repository = AndroidStreamingCredentialRepository(FakeCredentialStore(), FakeCredentialCipher())

        val settings: com.sona.android.application.recording.StreamingCredentialSettingsPort = repository
        val resolver: com.sona.android.application.recording.StreamingCredentialResolverPort = repository

        assertEquals(setOf("getStatus", "save", "clear"), settings.javaClass.interfaces
            .single { it.simpleName == "StreamingCredentialSettingsPort" }
            .declaredMethods.map { it.name }.toSet())
        assertTrue(resolver is com.sona.android.application.recording.StreamingCredentialResolverPort)
    }

    private suspend fun captureCredentialError(block: suspend () -> Unit): CredentialPersistenceException =
        try {
            block()
            throw AssertionError("Expected CredentialPersistenceException")
        } catch (error: CredentialPersistenceException) {
            error
        }

    private suspend fun captureCancellation(block: suspend () -> Unit): CancellationException =
        try {
            block()
            throw AssertionError("Expected CancellationException")
        } catch (error: CancellationException) {
            error
        }

    private companion object {
        fun supportedRecord(value: String): CredentialRecord {
            val plaintext = value.encodeToByteArray()
            return CredentialEnvelope(
                iv = ByteArray(12) { (it + 1).toByte() },
                ciphertext = ByteArray(16) + plaintext,
            ).toRecord()
        }
    }
}

private class FakeCredentialStore(
    initial: CredentialRecord = CredentialRecord(),
    private val operations: MutableList<String>? = null,
) : CredentialStore {
    private val state = MutableStateFlow(initial)
    override val records: Flow<CredentialRecord>
        get() = recordsFailure?.let { failure ->
            kotlinx.coroutines.flow.flow { throw failure }
        } ?: state
    var current: CredentialRecord
        get() = state.value
        private set(value) {
            state.value = value
        }
    var readFailure: Throwable? = null
    var writeFailure: Throwable? = null
    var clearFailure: Throwable? = null
    var recordsFailure: Throwable? = null
    var blockFirstRead: (suspend () -> Unit)? = null
    var operationDelayMillis = 0L
    var clearCalls = 0
    val maxConcurrentOperations = AtomicInteger(0)
    private val activeOperations = AtomicInteger(0)

    override suspend fun read(): CredentialRecord = operation("store.read") {
        blockFirstRead?.also { blockFirstRead = null }?.invoke()
        readFailure?.let { throw it }
        current
    }

    override suspend fun write(record: CredentialRecord) = operation("store.write") {
        writeFailure?.let { throw it }
        current = record
    }

    override suspend fun clear() = operation("store.clear") {
        clearCalls += 1
        clearFailure?.let { throw it }
        current = CredentialRecord()
    }

    private suspend fun <T> operation(name: String, block: suspend () -> T): T {
        val active = activeOperations.incrementAndGet()
        maxConcurrentOperations.updateAndGet { previous -> maxOf(previous, active) }
        operations?.add(name)
        return try {
            if (operationDelayMillis > 0) delay(operationDelayMillis)
            block()
        } finally {
            activeOperations.decrementAndGet()
        }
    }
}

private class FakeCredentialCipher(
    private val operations: MutableList<String>? = null,
) : CredentialCipher {
    var encryptCalls = 0
    var decryptCalls = 0
    var deleteCalls = 0
    var encryptFailure: Throwable? = null
    var decryptFailure: Throwable? = null
    var lastReceivedPlaintext: ByteArray? = null
    var lastReturnedPlaintext: ByteArray? = null

    override fun encrypt(plaintext: ByteArray): CredentialEnvelope {
        operations?.add("cipher.encrypt")
        encryptCalls += 1
        lastReceivedPlaintext = plaintext
        encryptFailure?.let { throw it }
        return CredentialEnvelope(
            iv = ByteArray(12) { index -> (encryptCalls + index).toByte() },
            ciphertext = ByteArray(16) + plaintext,
        )
    }

    override fun decrypt(envelope: CredentialEnvelope): ByteArray {
        operations?.add("cipher.decrypt")
        decryptCalls += 1
        decryptFailure?.let { throw it }
        return envelope.ciphertext.copyOfRange(16, envelope.ciphertext.size).also {
            lastReturnedPlaintext = it
        }
    }

    override fun deleteKey() {
        operations?.add("cipher.delete")
        deleteCalls += 1
    }
}
