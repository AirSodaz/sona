package com.sona.android.adapters.android.sync

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.UnrecoverableKeyException
import javax.crypto.AEADBadTagException
import javax.crypto.BadPaddingException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal data class AndroidKeyStoreSyncSecretPolicy(
    val alias: String,
    val provider: String,
    val algorithm: String,
    val transformation: String,
    val blockMode: String,
    val encryptionPadding: String,
    val keySizeBits: Int,
    val tagSizeBits: Int,
    val ivSizeBytes: Int,
    val encryptEnabled: Boolean,
    val decryptEnabled: Boolean,
    val randomizedEncryptionRequired: Boolean,
    val exportable: Boolean,
    private val aadPrefix: ByteArray,
) {
    fun aadFor(logicalKey: String): ByteArray = aadPrefix + logicalKey.encodeToByteArray()

    companion object {
        val production = AndroidKeyStoreSyncSecretPolicy(
            alias = "sona.sync_secrets.aes_gcm.v1",
            provider = "AndroidKeyStore",
            algorithm = "AES",
            transformation = "AES/GCM/NoPadding",
            blockMode = "GCM",
            encryptionPadding = "NoPadding",
            keySizeBits = 256,
            tagSizeBits = 128,
            ivSizeBytes = 12,
            encryptEnabled = true,
            decryptEnabled = true,
            randomizedEncryptionRequired = true,
            exportable = false,
            aadPrefix = "sona/android/sync-secret/v1/".encodeToByteArray(),
        )
    }
}

internal class AndroidKeyStoreSyncSecretCipher(
    private val policy: AndroidKeyStoreSyncSecretPolicy = AndroidKeyStoreSyncSecretPolicy.production,
) : SyncSecretCipher {
    override fun encrypt(logicalKey: String, plaintext: ByteArray): SyncSecretEnvelope = try {
        val cipher = Cipher.getInstance(policy.transformation)
        cipher.init(Cipher.ENCRYPT_MODE, loadOrCreateKey())
        val aad = policy.aadFor(logicalKey)
        try {
            cipher.updateAAD(aad)
        } finally {
            aad.fill(0)
        }
        val ciphertext = cipher.doFinal(plaintext)
        val iv = try {
            cipher.iv?.copyOf()
        } catch (error: Exception) {
            ciphertext.fill(0)
            throw error
        }
        if (iv == null) {
            ciphertext.fill(0)
            throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
        }
        if (iv.size != policy.ivSizeBytes || ciphertext.size < policy.tagSizeBits / 8) {
            iv.fill(0)
            ciphertext.fill(0)
            throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
        }
        SyncSecretEnvelope.fromTemporaryBuffers(iv, ciphertext)
    } catch (error: SyncSecretCipherException) {
        throw error
    } catch (_: KeyPermanentlyInvalidatedException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_KEY)
    } catch (_: UnrecoverableKeyException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_KEY)
    } catch (_: GeneralSecurityException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
    } catch (_: RuntimeException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
    } finally {
        plaintext.fill(0)
    }

    override fun decrypt(logicalKey: String, envelope: SyncSecretEnvelope): ByteArray = try {
        val key = loadExistingKey()
            ?: throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_KEY)
        val cipher = Cipher.getInstance(policy.transformation)
        val iv = envelope.iv
        val ciphertext = envelope.ciphertext
        try {
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(policy.tagSizeBits, iv))
            val aad = policy.aadFor(logicalKey)
            try {
                cipher.updateAAD(aad)
            } finally {
                aad.fill(0)
            }
            cipher.doFinal(ciphertext)
        } finally {
            iv.fill(0)
            ciphertext.fill(0)
        }
    } catch (error: SyncSecretCipherException) {
        throw error
    } catch (_: AEADBadTagException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_RECORD)
    } catch (_: BadPaddingException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_RECORD)
    } catch (_: KeyPermanentlyInvalidatedException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_KEY)
    } catch (_: UnrecoverableKeyException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.PERMANENT_KEY)
    } catch (_: GeneralSecurityException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
    } catch (_: RuntimeException) {
        throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
    }

    private fun loadOrCreateKey(): SecretKey {
        loadExistingKey()?.let { return it }
        val purposes = KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        val keySpec = KeyGenParameterSpec.Builder(policy.alias, purposes)
            .setKeySize(policy.keySizeBits)
            .setBlockModes(policy.blockMode)
            .setEncryptionPaddings(policy.encryptionPadding)
            .setRandomizedEncryptionRequired(policy.randomizedEncryptionRequired)
            .build()
        return KeyGenerator.getInstance(policy.algorithm, policy.provider).run {
            init(keySpec)
            generateKey()
        }
    }

    private fun loadExistingKey(): SecretKey? =
        loadKeyStore().getKey(policy.alias, null) as? SecretKey

    override fun resetAfterKeyInvalidation() {
        try {
            loadKeyStore().deleteEntry(policy.alias)
        } catch (_: GeneralSecurityException) {
            throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
        } catch (_: RuntimeException) {
            throw SyncSecretCipherException(SyncSecretCipherFailureKind.TEMPORARY)
        }
    }

    private fun loadKeyStore(): KeyStore = KeyStore.getInstance(policy.provider).apply { load(null) }
}
