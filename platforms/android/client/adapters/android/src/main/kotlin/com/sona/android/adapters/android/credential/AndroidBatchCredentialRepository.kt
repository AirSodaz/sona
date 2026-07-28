package com.sona.android.adapters.android.credential

import android.content.Context
import com.sona.android.application.recording.ActiveBatchCredential
import com.sona.android.application.recording.BatchCredentialConfiguration
import com.sona.android.application.recording.BatchCredentialResolverPort
import com.sona.android.application.recording.BatchCredentialSettingsPort
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineAsrProvider
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
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
    fun cipherFor(provider: OnlineAsrProvider): CredentialCipher
}

/**
 * Stores one online ASR API key per provider. Each provider owns an
 * independent Android Keystore alias, so saving or clearing one provider never
 * touches another. Plaintext leaves this class only through resolver loads.
 */
class AndroidBatchCredentialRepository internal constructor(
    private val store: BatchCredentialStore,
    private val ciphers: BatchCredentialCipherFactory,
    private val legacyStreamingCredential: LegacyStreamingCredentialSource? = null,
) : BatchCredentialSettingsPort, BatchCredentialResolverPort {
    private val operations = Mutex()
    @Volatile
    private var legacyMigrationComplete = legacyStreamingCredential == null

    override val configuration: Flow<BatchCredentialConfiguration> = flow {
        try {
            migrateLegacyStreamingCredential()
        } catch (error: CancellationException) {
            throw error
        } catch (_: BatchCredentialPersistenceException) {
            // Keep settings usable; runtime resolution retries the migration.
        }
        emitAll(store.records)
    }
        .map(::projectConfiguration)
        .distinctUntilChanged()
        .catch { error ->
            if (error is CancellationException) throw error
            if (error is BatchCredentialPersistenceException) throw error
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }

    override suspend fun selectProvider(provider: OnlineAsrProvider) {
        operations.withLock {
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
    }

    override suspend fun save(
        provider: OnlineAsrProvider,
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
        if (provider == DEFAULT_PROVIDER) {
            migrateLegacyStreamingCredential()
        }
    }

    override suspend fun clear(provider: OnlineAsrProvider) {
        operations.withLock {
            if (provider == DEFAULT_PROVIDER) {
                val legacy = legacyStreamingCredential
                if (legacy != null && !legacyMigrationComplete) {
                    clearLegacyCredentialOrThrow(legacy)
                    legacyMigrationComplete = true
                }
            }
            clearSlot(provider)
            deleteKey(provider)
        }
    }

    override suspend fun loadActive(): ActiveBatchCredential? {
        migrateLegacyStreamingCredential()
        return operations.withLock {
            val records = readRecords()
            val provider = selectedProvider(records)
            loadCredential(provider, records)
                ?.let { ActiveBatchCredential(provider = provider, credential = it) }
        }
    }

    override suspend fun load(provider: OnlineAsrProvider): OnlineBatchCredential? {
        migrateLegacyStreamingCredential()
        return operations.withLock {
            loadCredential(provider, readRecords())
        }
    }

    private suspend fun migrateLegacyStreamingCredential() {
        val legacy = legacyStreamingCredential ?: return
        if (legacyMigrationComplete) return

        operations.withLock {
            if (legacyMigrationComplete) return@withLock
            val targetRecord = readRecords().slotFor(DEFAULT_PROVIDER.storageId)
            val targetConfigured = when (CredentialEnvelope.inspect(targetRecord)) {
                CredentialEnvelopeState.Empty -> false
                CredentialEnvelopeState.Malformed -> {
                    cleanupInvalidSlot(DEFAULT_PROVIDER)
                    false
                }
                is CredentialEnvelopeState.Supported,
                is CredentialEnvelopeState.Unsupported,
                -> true
            }

            if (!targetConfigured) {
                val credential = loadLegacyCredential(legacy)
                if (credential != null) {
                    val plaintext = credential.apiKey.encodeToByteArray()
                    try {
                        if (credential.apiKey.isBlank() || plaintext.size > MAX_API_KEY_UTF8_BYTES) {
                            throw failure(CredentialErrorCode.INVALID_CREDENTIAL)
                        }
                        writeSlot(DEFAULT_PROVIDER, encrypt(DEFAULT_PROVIDER, plaintext).toRecord())
                    } finally {
                        plaintext.fill(0)
                    }
                }
            }

            if (tryClearLegacyCredential(legacy)) {
                legacyMigrationComplete = true
            }
        }
    }

    private suspend fun loadLegacyCredential(
        legacy: LegacyStreamingCredentialSource,
    ) = try {
        legacy.load()
    } catch (error: CancellationException) {
        throw error
    } catch (_: Exception) {
        throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
    }

    private suspend fun tryClearLegacyCredential(
        legacy: LegacyStreamingCredentialSource,
    ): Boolean = try {
        legacy.clear()
        true
    } catch (error: CancellationException) {
        throw error
    } catch (_: Exception) {
        false
    }

    private suspend fun clearLegacyCredentialOrThrow(
        legacy: LegacyStreamingCredentialSource,
    ) {
        try {
            legacy.clear()
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            throw failure(CredentialErrorCode.STORAGE_UNAVAILABLE)
        }
    }

    private suspend fun loadCredential(
        provider: OnlineAsrProvider,
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
        configuredProviders = OnlineAsrProvider.entries
            .filter {
                CredentialEnvelope.projectStatus(records.slotFor(it.storageId)) ==
                    CredentialStatus.CONFIGURED
            }
            .toSet(),
    )

    private fun selectedProvider(records: BatchCredentialRecords): OnlineAsrProvider =
        batchProviderForStorageId(records.selectedProviderStorageId) ?: DEFAULT_PROVIDER

    private suspend fun loadSupported(
        provider: OnlineAsrProvider,
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
        provider: OnlineAsrProvider,
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

    private suspend fun cleanupInvalidSlot(provider: OnlineAsrProvider) {
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

    private suspend fun readSlot(provider: OnlineAsrProvider): CredentialRecord =
        readRecords().slotFor(provider.storageId)

    private suspend fun writeSlot(provider: OnlineAsrProvider, record: CredentialRecord) {
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

    private suspend fun clearSlot(provider: OnlineAsrProvider) {
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

    private fun deleteKey(provider: OnlineAsrProvider) {
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
        private val DEFAULT_PROVIDER = OnlineAsrProvider.VOLCENGINE_DOUBAO

        @JvmStatic
        fun create(context: Context): AndroidBatchCredentialRepository = try {
            AndroidBatchCredentialRepository(
                store = BatchCredentialDataStore.create(context.applicationContext),
                ciphers = { provider ->
                    AndroidKeyStoreCredentialCipher(
                        AndroidKeyStoreCredentialPolicy.batch(provider.storageId),
                    )
                },
                legacyStreamingCredential =
                    LegacyStreamingCredentialRepository.createIfPresent(context.applicationContext),
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
