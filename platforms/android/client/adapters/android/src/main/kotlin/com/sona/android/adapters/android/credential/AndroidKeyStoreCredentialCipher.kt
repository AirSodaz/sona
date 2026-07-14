package com.sona.android.adapters.android.credential

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

internal data class AndroidKeyStoreCredentialPolicy(
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
    private val aadValue: ByteArray,
) {
    val aad: ByteArray
        get() = aadValue.copyOf()

    companion object {
        val production = AndroidKeyStoreCredentialPolicy(
            alias = "sona.streaming_credential.aes_gcm.v1",
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
            aadValue = "sona/android/streaming-credential/v1".encodeToByteArray(),
        )
    }
}

internal class AndroidKeyStoreCredentialCipher(
    private val policy: AndroidKeyStoreCredentialPolicy = AndroidKeyStoreCredentialPolicy.production,
) : CredentialCipher {
    override fun encrypt(plaintext: ByteArray): CredentialEnvelope = try {
        val cipher = Cipher.getInstance(policy.transformation)
        cipher.init(Cipher.ENCRYPT_MODE, loadOrCreateKey())
        val aad = policy.aad
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
            throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
        }
        if (iv.size != policy.ivSizeBytes || ciphertext.size < policy.tagSizeBits / 8) {
            iv.fill(0)
            ciphertext.fill(0)
            throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
        }
        CredentialEnvelope.fromTemporaryBuffers(iv = iv, ciphertext = ciphertext)
    } catch (error: CredentialCipherException) {
        throw error
    } catch (_: KeyPermanentlyInvalidatedException) {
        throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
    } catch (_: UnrecoverableKeyException) {
        throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
    } catch (_: GeneralSecurityException) {
        throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
    } catch (_: RuntimeException) {
        throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
    } finally {
        plaintext.fill(0)
    }

    override fun decrypt(envelope: CredentialEnvelope): ByteArray = try {
        val key = loadExistingKey()
            ?: throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
        val cipher = Cipher.getInstance(policy.transformation)
        val iv = envelope.iv
        val ciphertext = envelope.ciphertext
        try {
            cipher.init(
                Cipher.DECRYPT_MODE,
                key,
                GCMParameterSpec(policy.tagSizeBits, iv),
            )
            val aad = policy.aad
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
    } catch (error: CredentialCipherException) {
        throw error
    } catch (_: AEADBadTagException) {
        throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
    } catch (_: BadPaddingException) {
        throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
    } catch (_: KeyPermanentlyInvalidatedException) {
        throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
    } catch (_: UnrecoverableKeyException) {
        throw CredentialCipherException(CredentialCipherFailureKind.PERMANENT)
    } catch (_: GeneralSecurityException) {
        throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
    } catch (_: RuntimeException) {
        throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
    }

    override fun deleteKey() {
        try {
            loadKeyStore().run {
                if (containsAlias(policy.alias)) {
                    deleteEntry(policy.alias)
                }
            }
        } catch (_: GeneralSecurityException) {
            throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
        } catch (_: RuntimeException) {
            throw CredentialCipherException(CredentialCipherFailureKind.TEMPORARY)
        }
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

    private fun loadKeyStore(): KeyStore = KeyStore.getInstance(policy.provider).apply { load(null) }
}
