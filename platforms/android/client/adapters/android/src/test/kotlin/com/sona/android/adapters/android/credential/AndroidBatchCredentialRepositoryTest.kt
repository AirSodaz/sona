package com.sona.android.adapters.android.credential

import com.sona.android.application.recording.ActiveBatchCredential
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineBatchProvider
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidBatchCredentialRepositoryTest {
    @Test
    fun `configuration reports every configured provider without decrypting`() = runBlocking {
        val ciphers = FakeSlotCipherFactory().apply {
            decryptFailure = AssertionError("must not decrypt")
        }
        val store = FakeBatchStore(
            BatchCredentialRecords(
                selectedProviderStorageId = "groq-whisper",
                slots = mapOf(
                    "groq-whisper" to supportedRecord("groq-secret"),
                    "mistral-voxtral" to supportedRecord("mistral-secret"),
                ),
            ),
        )
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        val configuration = repository.configuration.first()

        assertEquals(OnlineBatchProvider.GROQ_WHISPER, configuration.selectedProvider)
        assertEquals(
            setOf(OnlineBatchProvider.GROQ_WHISPER, OnlineBatchProvider.MISTRAL_VOXTRAL),
            configuration.configuredProviders,
        )
        assertEquals(CredentialStatus.CONFIGURED, configuration.selectedStatus)
        assertEquals(
            CredentialStatus.NOT_CONFIGURED,
            configuration.statusFor(OnlineBatchProvider.VOLCENGINE_DOUBAO),
        )
        assertEquals(0, ciphers.decryptCalls)
    }

    @Test
    fun `an absent or unknown selection falls back to the default provider`() = runBlocking {
        val unknown = FakeBatchStore(BatchCredentialRecords(selectedProviderStorageId = "unknown"))
        val absent = FakeBatchStore(BatchCredentialRecords())

        listOf(unknown, absent).forEach { store ->
            val repository = AndroidBatchCredentialRepository(store, FakeSlotCipherFactory())

            assertEquals(
                OnlineBatchProvider.VOLCENGINE_DOUBAO,
                repository.configuration.first().selectedProvider,
            )
        }
    }

    @Test
    fun `the active provider round trips and decrypted bytes are cleared`() = runBlocking {
        val ciphers = FakeSlotCipherFactory()
        val store = FakeBatchStore()
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        repository.selectProvider(OnlineBatchProvider.GROQ_WHISPER)
        repository.save(OnlineBatchProvider.GROQ_WHISPER, OnlineBatchCredential("groq-secret"))
        val loaded = repository.loadActive()

        assertEquals(
            ActiveBatchCredential(
                provider = OnlineBatchProvider.GROQ_WHISPER,
                credential = OnlineBatchCredential("groq-secret"),
            ),
            loaded,
        )
        assertTrue(ciphers.lastReturnedPlaintext!!.all { it == 0.toByte() })
        assertTrue(ciphers.lastReceivedPlaintext!!.all { it == 0.toByte() })
    }

    @Test
    fun `saving one provider never touches another provider slot or alias`() = runBlocking {
        val operations = mutableListOf<String>()
        val ciphers = FakeSlotCipherFactory(operations)
        val store = FakeBatchStore(
            BatchCredentialRecords(slots = mapOf("groq-whisper" to supportedRecord("groq-secret"))),
            operations,
        )
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        repository.save(
            OnlineBatchProvider.MISTRAL_VOXTRAL,
            OnlineBatchCredential("mistral-secret"),
        )

        assertEquals(supportedRecord("groq-secret"), store.current.slots["groq-whisper"])
        assertTrue(store.current.slots.containsKey("mistral-voxtral"))
        assertEquals(0, ciphers.cipherFor(OnlineBatchProvider.GROQ_WHISPER).deleteCalls)
        assertFalse(operations.any { it.startsWith("store.clear.groq-whisper") })
    }

    @Test
    fun `clearing one provider keeps every other provider readable`() = runBlocking {
        val ciphers = FakeSlotCipherFactory()
        val store = FakeBatchStore(
            BatchCredentialRecords(
                selectedProviderStorageId = "groq-whisper",
                slots = mapOf(
                    "groq-whisper" to supportedRecord("groq-secret"),
                    "mistral-voxtral" to supportedRecord("mistral-secret"),
                ),
            ),
        )
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        repository.clear(OnlineBatchProvider.GROQ_WHISPER)

        assertNull(store.current.slots["groq-whisper"])
        assertEquals(1, ciphers.cipherFor(OnlineBatchProvider.GROQ_WHISPER).deleteCalls)
        assertEquals(0, ciphers.cipherFor(OnlineBatchProvider.MISTRAL_VOXTRAL).deleteCalls)
        assertNull(repository.loadActive())

        repository.selectProvider(OnlineBatchProvider.MISTRAL_VOXTRAL)
        assertEquals(
            OnlineBatchCredential("mistral-secret"),
            repository.loadActive()?.credential,
        )
    }

    @Test
    fun `blank and oversized keys are rejected with a fixed public error`() = runBlocking {
        val store = FakeBatchStore()
        val repository = AndroidBatchCredentialRepository(store, FakeSlotCipherFactory())

        listOf("", " \t\n", "a".repeat(16_385)).forEach { apiKey ->
            val error = captureError {
                repository.save(OnlineBatchProvider.GROQ_WHISPER, OnlineBatchCredential(apiKey))
            }
            assertEquals(CredentialErrorCode.INVALID_CREDENTIAL, error.code)
            assertEquals("Cloud transcription credential is invalid.", error.message)
            assertNull(error.cause)
        }
        assertEquals(BatchCredentialRecords(), store.current)
    }

    @Test
    fun `a malformed slot is cleaned up and reported as unconfigured`() = runBlocking {
        val operations = mutableListOf<String>()
        val malformed = CredentialRecord(formatVersion = 1, ivBase64 = "partial")
        val store = FakeBatchStore(
            BatchCredentialRecords(
                selectedProviderStorageId = "groq-whisper",
                slots = mapOf("groq-whisper" to malformed),
            ),
            operations,
        )
        val ciphers = FakeSlotCipherFactory(operations)
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        assertEquals(
            emptySet<OnlineBatchProvider>(),
            repository.configuration.first().configuredProviders,
        )
        assertNull(repository.loadActive())

        assertEquals(
            listOf("store.read", "store.clear.groq-whisper", "cipher.delete.groq-whisper"),
            operations,
        )
        assertNull(store.current.slots["groq-whisper"])
    }

    @Test
    fun `an unknown slot version blocks load and save without discarding it`() = runBlocking {
        val unknown = CredentialRecord(formatVersion = 7)
        val store = FakeBatchStore(
            BatchCredentialRecords(
                selectedProviderStorageId = "groq-whisper",
                slots = mapOf("groq-whisper" to unknown),
            ),
        )
        val ciphers = FakeSlotCipherFactory()
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        val loadError = captureError { repository.loadActive() }
        val saveError = captureError {
            repository.save(OnlineBatchProvider.GROQ_WHISPER, OnlineBatchCredential("replacement"))
        }

        assertEquals(CredentialErrorCode.UNSUPPORTED_FORMAT, loadError.code)
        assertEquals(CredentialErrorCode.UNSUPPORTED_FORMAT, saveError.code)
        assertEquals(unknown, store.current.slots["groq-whisper"])
        assertEquals(0, ciphers.cipherFor(OnlineBatchProvider.GROQ_WHISPER).encryptCalls)
        assertEquals(0, ciphers.cipherFor(OnlineBatchProvider.GROQ_WHISPER).deleteCalls)
    }

    @Test
    fun `a permanent decrypt failure rotates only that provider and returns null`() = runBlocking {
        val ciphers = FakeSlotCipherFactory().apply {
            decryptFailure = CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
        }
        val store = FakeBatchStore(
            BatchCredentialRecords(
                selectedProviderStorageId = "groq-whisper",
                slots = mapOf(
                    "groq-whisper" to supportedRecord("groq-secret"),
                    "mistral-voxtral" to supportedRecord("mistral-secret"),
                ),
            ),
        )
        val repository = AndroidBatchCredentialRepository(store, ciphers)

        assertNull(repository.loadActive())

        assertNull(store.current.slots["groq-whisper"])
        assertEquals(
            supportedRecord("mistral-secret"),
            store.current.slots["mistral-voxtral"],
        )
        assertEquals(0, ciphers.cipherFor(OnlineBatchProvider.MISTRAL_VOXTRAL).deleteCalls)
    }

    @Test
    fun `storage failures stay redacted and preserve the stored slots`() = runBlocking {
        val sentinel = "sentinel-secret /private/path alias-value"
        val initial = BatchCredentialRecords(
            slots = mapOf("groq-whisper" to supportedRecord("groq-secret")),
        )
        val store = FakeBatchStore(initial).apply {
            readFailure = IllegalStateException(sentinel)
        }
        val repository = AndroidBatchCredentialRepository(store, FakeSlotCipherFactory())

        val error = captureError { repository.loadActive() }

        assertEquals(CredentialErrorCode.STORAGE_UNAVAILABLE, error.code)
        assertEquals("Cloud transcription credential storage is unavailable.", error.message)
        assertFalse(error.toString().contains(sentinel))
        assertFalse(error.stackTraceToString().contains(sentinel))
        assertNull(error.cause)
        assertEquals(initial, store.current)
    }

    @Test
    fun `cancellation is rethrown by save load and clear without mutation`() = runBlocking {
        val initial = BatchCredentialRecords(
            slots = mapOf("groq-whisper" to supportedRecord("groq-secret")),
        )

        val saveCancellation = CancellationException("save-cancelled")
        val saveStore = FakeBatchStore(initial).apply { writeFailure = saveCancellation }
        val saveRepository = AndroidBatchCredentialRepository(saveStore, FakeSlotCipherFactory())
        assertTrue(
            captureCancellation {
                saveRepository.save(
                    OnlineBatchProvider.GROQ_WHISPER,
                    OnlineBatchCredential("replacement"),
                )
            } === saveCancellation,
        )
        assertEquals(initial, saveStore.current)

        val loadCancellation = CancellationException("load-cancelled")
        val loadStore = FakeBatchStore(initial).apply { readFailure = loadCancellation }
        val loadRepository = AndroidBatchCredentialRepository(loadStore, FakeSlotCipherFactory())
        assertTrue(captureCancellation { loadRepository.loadActive() } === loadCancellation)
        assertEquals(initial, loadStore.current)

        val clearCancellation = CancellationException("clear-cancelled")
        val clearStore = FakeBatchStore(initial).apply { clearFailure = clearCancellation }
        val clearCiphers = FakeSlotCipherFactory()
        val clearRepository = AndroidBatchCredentialRepository(clearStore, clearCiphers)
        assertTrue(
            captureCancellation {
                clearRepository.clear(OnlineBatchProvider.GROQ_WHISPER)
            } === clearCancellation,
        )
        assertEquals(initial, clearStore.current)
        assertEquals(0, clearCiphers.cipherFor(OnlineBatchProvider.GROQ_WHISPER).deleteCalls)
    }

    @Test
    fun `every provider maps to a stable storage id`() {
        assertEquals(
            listOf("volcengine-doubao", "groq-whisper", "mistral-voxtral"),
            OnlineBatchProvider.entries.map { it.storageId },
        )
        OnlineBatchProvider.entries.forEach { provider ->
            assertEquals(provider, batchProviderForStorageId(provider.storageId))
        }
        assertNull(batchProviderForStorageId("unknown"))
        assertNull(batchProviderForStorageId(null))
    }

    private suspend fun captureError(
        block: suspend () -> Unit,
    ): BatchCredentialPersistenceException = try {
        block()
        throw AssertionError("Expected BatchCredentialPersistenceException")
    } catch (error: BatchCredentialPersistenceException) {
        error
    }

    private suspend fun captureCancellation(block: suspend () -> Unit): CancellationException = try {
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

private class FakeBatchStore(
    initial: BatchCredentialRecords = BatchCredentialRecords(),
    private val operations: MutableList<String>? = null,
) : BatchCredentialStore {
    private val state = MutableStateFlow(initial)

    override val records: Flow<BatchCredentialRecords>
        get() = recordsFailure?.let { failure -> flow<BatchCredentialRecords> { throw failure } }
            ?: state

    val current: BatchCredentialRecords
        get() = state.value

    var readFailure: Throwable? = null
    var writeFailure: Throwable? = null
    var clearFailure: Throwable? = null
    var recordsFailure: Throwable? = null

    override suspend fun read(): BatchCredentialRecords {
        operations?.add("store.read")
        readFailure?.let { throw it }
        return current
    }

    override suspend fun writeSlot(storageId: String, record: CredentialRecord) {
        operations?.add("store.write.$storageId")
        writeFailure?.let { throw it }
        state.value = current.copy(slots = current.slots + (storageId to record))
    }

    override suspend fun clearSlot(storageId: String) {
        operations?.add("store.clear.$storageId")
        clearFailure?.let { throw it }
        state.value = current.copy(slots = current.slots - storageId)
    }

    override suspend fun writeSelectedProvider(storageId: String) {
        operations?.add("store.select.$storageId")
        writeFailure?.let { throw it }
        state.value = current.copy(selectedProviderStorageId = storageId)
    }
}

private class FakeSlotCipherFactory(
    private val operations: MutableList<String>? = null,
) : BatchCredentialCipherFactory {
    private val ciphers = mutableMapOf<OnlineBatchProvider, FakeSlotCipher>()
    var decryptFailure: Throwable? = null
    var lastReceivedPlaintext: ByteArray? = null
    var lastReturnedPlaintext: ByteArray? = null

    val decryptCalls: Int
        get() = ciphers.values.sumOf { it.decryptCalls }

    override fun cipherFor(provider: OnlineBatchProvider): FakeSlotCipher =
        ciphers.getOrPut(provider) { FakeSlotCipher(this, provider, operations) }
}

private class FakeSlotCipher(
    private val factory: FakeSlotCipherFactory,
    private val provider: OnlineBatchProvider,
    private val operations: MutableList<String>?,
) : CredentialCipher {
    var encryptCalls = 0
    var decryptCalls = 0
    var deleteCalls = 0

    override fun encrypt(plaintext: ByteArray): CredentialEnvelope {
        operations?.add("cipher.encrypt.${provider.storageId}")
        encryptCalls += 1
        factory.lastReceivedPlaintext = plaintext
        return CredentialEnvelope(
            iv = ByteArray(12) { index -> (encryptCalls + index).toByte() },
            ciphertext = ByteArray(16) + plaintext,
        ).also { plaintext.fill(0) }
    }

    override fun decrypt(envelope: CredentialEnvelope): ByteArray {
        operations?.add("cipher.decrypt.${provider.storageId}")
        decryptCalls += 1
        factory.decryptFailure?.let { throw it }
        return envelope.ciphertext.copyOfRange(16, envelope.ciphertext.size).also {
            factory.lastReturnedPlaintext = it
        }
    }

    override fun deleteKey() {
        operations?.add("cipher.delete.${provider.storageId}")
        deleteCalls += 1
    }
}
