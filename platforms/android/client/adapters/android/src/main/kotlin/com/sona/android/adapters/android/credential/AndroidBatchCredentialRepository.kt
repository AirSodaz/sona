package com.sona.android.adapters.android.credential

import android.content.Context
import com.sona.android.application.recording.ActiveBatchCredential
import com.sona.android.application.recording.BatchCredentialConfiguration
import com.sona.android.application.recording.BatchCredentialResolverPort
import com.sona.android.application.recording.BatchCredentialSettingsPort
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineBatchProvider
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

class BatchCredentialPersistenceException internal constructor(
    val code: CredentialErrorCode,
) : IllegalStateException(
    when (code) {
        CredentialErrorCode.INVALID_CREDENTIAL -> "Cloud transcription credential is invalid."
        CredentialErrorCode.UNSUPPORTED_FORMAT ->
            "Cloud transcription credential format is unsupported."
        CredentialErrorCode.STORAGE_UNAVAILABLE ->
            "Cloud transcription credential storage is unavailable."
    },
)

internal fun interface BatchCredentialCipherFactory {
    fun cipherFor(provider: OnlineBatchProvider): CredentialCipher
}

/**
 * Stores one cloud batch API key per provider. Each provider owns an
 * independent Android Keystore alias, so saving or clearing one provider never
 * touches another. Plaintext leaves this class only through [loadActive].
 */
class AndroidBatchCredentialRepository internal constructor(
    private val store: BatchCredentialStore,
    private val ciphers: BatchCredentialCipherFactory,
) : BatchCredentialSettingsPort, BatchCredentialResolverPort {
    private val operations = Mutex()

    override val configuration: Flow<BatchCredentialConfiguration> = store.records
        .map(::projectConfiguration)
        .distinctUntilChanged()
        .catch { error ->
            if (error is CancellationException) throw error
            if (error is BatchCredentialPersistenceException) throw error
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }

    override suspend fun selectProvider(provider: OnlineBatchProvider) = operations.withLock {
        try {
            store.writeSelectedProvider(provider.storageId)
        } catch (error: CancellationException) {
            throw error
        } catch (error: BatchCredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    override suspend fun save(
        provider: OnlineBatchProvider,
        credential: OnlineBatchCredential,
    ) {
        val plaintext = credential.apiKey.encodeToByteArray()
        try {
            if (credential.apiKey.isBlank() || plaintext.size > MAX_API_KEY_UTF8_BYTES) {
                throw failure(CredentialErrorCode.INVALID_CREDENTIAL)
            }
            operations.withLock {
                when (CredentialEnvelope.inspect(readSlot(provider))) {
                    CredentialEnvelopeState.Empty -> deleteKey(provider)
                    CredentialEnvelopeState.Malformed -> cleanupInvalidSlot(provider)
                    is CredentialEnvelopeState.Unsupported -> {
                        throw failure(CredentialErrorCode.UNSUPPORTED_FORMAT)
                    }
                    is CredentialEnvelopeState.Supported -> Unit
                }
                writeSlot(provider, encrypt(provider, plaintext).toRecord())
            }
        } finally {
            plaintext.fill(0)
        }
    }

    override suspend fun clear(provider: OnlineBatchProvider) = operations.withLock {
        clearSlot(provider)
        deleteKey(provider)
    }

    override suspend fun loadActive(): ActiveBatchCredential? = operations.withLock {
        val records = readRecords()
        val provider = selectedProvider(records)
        loadCredential(provider, records)
            ?.let { ActiveBatchCredential(provider = provider, credential = it) }
    }

    override suspend fun load(provider: OnlineBatchProvider): OnlineBatchCredential? =
        operations.withLock {
            loadCredential(provider, readRecords())
        }

    private suspend fun loadCredential(
        provider: OnlineBatchProvider,
        records: BatchCredentialRecords,
    ): OnlineBatchCredential? {
        val slot = records.slotFor(provider.storageId)
        val credential = when (val state = CredentialEnvelope.inspect(slot)) {
            CredentialEnvelopeState.Empty -> null
            CredentialEnvelopeState.Malformed -> {
                cleanupInvalidSlot(provider)
                null
            }
            is CredentialEnvelopeState.Unsupported -> {
                throw failure(CredentialErrorCode.UNSUPPORTED_FORMAT)
            }
            is CredentialEnvelopeState.Supported -> loadSupported(provider, state.envelope)
        }
        return credential
    }

    private fun projectConfiguration(records: BatchCredentialRecords) = BatchCredentialConfiguration(
        selectedProvider = selectedProvider(records),
        configuredProviders = OnlineBatchProvider.entries
            .filter {
                CredentialEnvelope.projectStatus(records.slotFor(it.storageId)) ==
                    CredentialStatus.CONFIGURED
            }
            .toSet(),
    )

    private fun selectedProvider(records: BatchCredentialRecords): OnlineBatchProvider =
        batchProviderForStorageId(records.selectedProviderStorageId) ?: DEFAULT_PROVIDER

    private suspend fun loadSupported(
        provider: OnlineBatchProvider,
        envelope: CredentialEnvelope,
    ): OnlineBatchCredential? {
        val plaintext = try {
            ciphers.cipherFor(provider).decrypt(envelope)
        } catch (error: CredentialCipherException) {
            if (error.kind == CredentialCipherFailureKind.PERMANENT) {
                cleanupInvalidSlot(provider)
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
                cleanupInvalidSlot(provider)
                null
            } else {
                OnlineBatchCredential(apiKey)
            }
        } finally {
            plaintext.fill(0)
        }
    }

    private suspend fun encrypt(
        provider: OnlineBatchProvider,
        plaintext: ByteArray,
    ): CredentialEnvelope = try {
        ciphers.cipherFor(provider).encrypt(plaintext)
    } catch (error: CancellationException) {
        throw error
    } catch (error: CredentialCipherException) {
        if (error.kind == CredentialCipherFailureKind.PERMANENT) {
            cleanupInvalidSlot(provider)
        }
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    } catch (_: Exception) {
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    }

    private suspend fun cleanupInvalidSlot(provider: OnlineBatchProvider) {
        clearSlot(provider)
        deleteKey(provider)
    }

    private suspend fun readRecords(): BatchCredentialRecords = try {
        store.read()
    } catch (error: CancellationException) {
        throw error
    } catch (error: BatchCredentialPersistenceException) {
        throw error
    } catch (_: Exception) {
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    }

    private suspend fun readSlot(provider: OnlineBatchProvider): CredentialRecord =
        readRecords().slotFor(provider.storageId)

    private suspend fun writeSlot(provider: OnlineBatchProvider, record: CredentialRecord) {
        try {
            store.writeSlot(provider.storageId, record)
        } catch (error: CancellationException) {
            throw error
        } catch (error: BatchCredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    private suspend fun clearSlot(provider: OnlineBatchProvider) {
        try {
            store.clearSlot(provider.storageId)
        } catch (error: CancellationException) {
            throw error
        } catch (error: BatchCredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    private fun deleteKey(provider: OnlineBatchProvider) {
        try {
            ciphers.cipherFor(provider).deleteKey()
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    companion object {
        private const val MAX_API_KEY_UTF8_BYTES = 16_384
        private val DEFAULT_PROVIDER = OnlineBatchProvider.VOLCENGINE_DOUBAO

        @JvmStatic
        fun create(context: Context): AndroidBatchCredentialRepository = try {
            AndroidBatchCredentialRepository(
                store = BatchCredentialDataStore.create(context.applicationContext),
                ciphers = { provider ->
                    AndroidKeyStoreCredentialCipher(
                        AndroidKeyStoreCredentialPolicy.batch(provider.storageId),
                    )
                },
            )
        } catch (error: BatchCredentialPersistenceException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }

        private fun failure(code: CredentialErrorCode) = BatchCredentialPersistenceException(code)

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
