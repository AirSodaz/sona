package com.sona.android.adapters.android.credential

import android.content.Context
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.StreamingCredential
import com.sona.android.application.recording.StreamingCredentialResolverPort
import com.sona.android.application.recording.StreamingCredentialSettingsPort
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

enum class CredentialErrorCode {
    INVALID_CREDENTIAL,
    UNSUPPORTED_FORMAT,
    STORAGE_UNAVAILABLE,
}

class CredentialPersistenceException internal constructor(
    val code: CredentialErrorCode,
) : IllegalStateException(
    when (code) {
        CredentialErrorCode.INVALID_CREDENTIAL -> "Streaming credential is invalid."
        CredentialErrorCode.UNSUPPORTED_FORMAT -> "Streaming credential format is unsupported."
        CredentialErrorCode.STORAGE_UNAVAILABLE -> "Streaming credential storage is unavailable."
    },
)

class AndroidStreamingCredentialRepository internal constructor(
    private val store: CredentialStore,
    private val cipher: CredentialCipher,
) : StreamingCredentialSettingsPort, StreamingCredentialResolverPort {
    private val operations = Mutex()

    override val status: Flow<CredentialStatus> = store.records
        .map(CredentialEnvelope::projectStatus)
        .distinctUntilChanged()
        .catch { error ->
            if (error is CancellationException) throw error
            if (error is CredentialPersistenceException) throw error
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }

    override suspend fun save(credential: StreamingCredential) {
        val plaintext = credential.apiKey.encodeToByteArray()
        try {
            if (credential.apiKey.isBlank() || plaintext.size > MAX_API_KEY_UTF8_BYTES) {
                throw failure(CredentialErrorCode.INVALID_CREDENTIAL)
            }
            operations.withLock {
                val current = readRecord()
                when (CredentialEnvelope.inspect(current)) {
                    CredentialEnvelopeState.Empty -> deleteKey()
                    CredentialEnvelopeState.Malformed -> cleanupInvalidRecord()
                    is CredentialEnvelopeState.Unsupported -> {
                        throw failure(CredentialErrorCode.UNSUPPORTED_FORMAT)
                    }
                    is CredentialEnvelopeState.Supported -> Unit
                }
                val envelope = encrypt(plaintext)
                writeRecord(envelope.toRecord())
            }
        } finally {
            plaintext.fill(0)
        }
    }

    override suspend fun loadForStart(): StreamingCredential? = operations.withLock {
        when (val state = CredentialEnvelope.inspect(readRecord())) {
            CredentialEnvelopeState.Empty -> null
            CredentialEnvelopeState.Malformed -> {
                cleanupInvalidRecord()
                null
            }
            is CredentialEnvelopeState.Unsupported -> {
                throw failure(CredentialErrorCode.UNSUPPORTED_FORMAT)
            }
            is CredentialEnvelopeState.Supported -> loadSupported(state.envelope)
        }
    }

    override suspend fun clear() = operations.withLock {
        clearRecord()
        deleteKey()
    }

    private suspend fun loadSupported(envelope: CredentialEnvelope): StreamingCredential? {
        val plaintext = try {
            cipher.decrypt(envelope)
        } catch (error: CredentialCipherException) {
            if (error.kind == CredentialCipherFailureKind.PERMANENT) {
                cleanupInvalidRecord()
                return null
            }
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
        return try {
            val apiKey = decodeUtf8(plaintext)
            if (apiKey == null || apiKey.isBlank() || plaintext.size > MAX_API_KEY_UTF8_BYTES) {
                cleanupInvalidRecord()
                null
            } else {
                StreamingCredential(apiKey)
            }
        } finally {
            plaintext.fill(0)
        }
    }

    private suspend fun encrypt(plaintext: ByteArray): CredentialEnvelope = try {
        cipher.encrypt(plaintext)
    } catch (error: CancellationException) {
        throw error
    } catch (error: CredentialCipherException) {
        if (error.kind == CredentialCipherFailureKind.PERMANENT) {
            cleanupInvalidRecord()
        }
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    } catch (_: Exception) {
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    }

    private suspend fun cleanupInvalidRecord() {
        clearRecord()
        deleteKey()
    }

    private suspend fun readRecord(): CredentialRecord = try {
        store.read()
    } catch (error: CancellationException) {
        throw error
    } catch (error: CredentialPersistenceException) {
        throw error
    } catch (_: Exception) {
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    }

    private suspend fun writeRecord(record: CredentialRecord) {
        try {
            store.write(record)
        } catch (error: CancellationException) {
            throw error
        } catch (error: CredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    private suspend fun clearRecord() {
        try {
            store.clear()
        } catch (error: CancellationException) {
            throw error
        } catch (error: CredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    private fun deleteKey() {
        try {
            cipher.deleteKey()
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    companion object {
        private const val MAX_API_KEY_UTF8_BYTES = 16_384

        @JvmStatic
        fun create(context: Context): AndroidStreamingCredentialRepository = try {
            AndroidStreamingCredentialRepository(
                store = CredentialDataStore.create(context.applicationContext),
                cipher = AndroidKeyStoreCredentialCipher(),
            )
        } catch (error: CredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }

        private fun failure(code: CredentialErrorCode) = CredentialPersistenceException(code)

        private fun decodeUtf8(bytes: ByteArray): String? = try {
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString()
        } catch (_: Exception) {
            null
        }
    }
}
