package com.sona.android.adapters.android.credential

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.StreamingCredential
import java.io.File
import java.security.KeyStore
import java.util.UUID
import javax.crypto.SecretKey
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AndroidStreamingCredentialRepositoryInstrumentedTest {
    private lateinit var context: Context
    private lateinit var alias: String
    private lateinit var fileName: String
    private var activeStore: CredentialDataStore? = null

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        val uniqueId = UUID.randomUUID().toString()
        alias = "sona.streaming_credential.test.$uniqueId"
        fileName = "streaming_credentials_test_$uniqueId.preferences_pb"
        deleteAlias(alias)
        testFile().delete()
    }

    @After
    fun tearDown() = runBlocking {
        activeStore?.closeAndAwait()
        activeStore = null
        deleteAlias(alias)
        testFile().delete()
    }

    @Test
    fun realSaveLoadOverwriteAndClearUseANonExportableKeyAndFreshEnvelope() = runBlocking {
        val store = openStore()
        val repository = repository(store)

        repository.save(StreamingCredential("device-secret"))
        val firstRecord = store.read()
        val key = keyStore().getKey(alias, null) as SecretKey

        assertNull(key.encoded)
        assertEquals(StreamingCredential("device-secret"), repository.loadForStart())
        repository.save(StreamingCredential("device-secret"))
        val secondRecord = store.read()
        assertNotEquals(firstRecord.ivBase64, secondRecord.ivBase64)
        assertNotEquals(firstRecord.ciphertextBase64, secondRecord.ciphertextBase64)

        repository.clear()
        repository.clear()

        assertNull(repository.loadForStart())
        assertEquals(CredentialStatus.NOT_CONFIGURED, repository.status.first())
        assertEquals(CredentialRecord(), store.read())
        assertFalse(keyStore().containsAlias(alias))
    }

    @Test
    fun missingAliasClearsEnvelopeAndNextSaveRecreatesKey() = runBlocking {
        val store = openStore()
        val repository = repository(store)
        repository.save(StreamingCredential("before-loss"))
        deleteAlias(alias)

        assertNull(repository.loadForStart())
        assertEquals(CredentialRecord(), store.read())
        repository.save(StreamingCredential("after-loss"))

        assertEquals(StreamingCredential("after-loss"), repository.loadForStart())
        assertTrue(keyStore().containsAlias(alias))
    }

    @Test
    fun tamperedCiphertextClearsEnvelopeBeforeDeletingAlias() = runBlocking {
        val store = openStore()
        val repository = repository(store)
        repository.save(StreamingCredential("tamper-target"))
        val supported = CredentialEnvelope.inspect(store.read()) as CredentialEnvelopeState.Supported
        val tamperedCiphertext = supported.envelope.ciphertext.copyOf().also { bytes ->
            bytes[0] = (bytes[0].toInt() xor 0x01).toByte()
        }
        store.write(CredentialEnvelope(supported.envelope.iv, tamperedCiphertext).toRecord())

        assertNull(repository.loadForStart())

        assertEquals(CredentialRecord(), store.read())
        assertFalse(keyStore().containsAlias(alias))
    }

    @Test
    fun corruptionHandlerRestoresEmptyPreferencesAfterScopeCancellation() = runBlocking {
        val first = openStore()
        first.write(CredentialEnvelope(ByteArray(12), ByteArray(16)).toRecord())
        first.closeAndAwait()
        activeStore = null
        testFile().writeBytes(byteArrayOf(0x80.toByte()))

        val reopened = openStore()

        assertEquals(CredentialRecord(), reopened.read())
    }

    @Test
    fun dataStoreUsesCanonicalNoBackupPathAndAllowsOnlyOneActiveInstance() = runBlocking {
        assertEquals(
            File(context.noBackupFilesDir, "streaming_credentials.preferences_pb").canonicalFile,
            CredentialDataStore.defaultFile(context).canonicalFile,
        )
        val first = openStore()

        val duplicateError = assertThrows(IllegalStateException::class.java) {
            CredentialDataStore.createForTesting(context, fileName)
        }
        assertEquals("Credential DataStore is already active.", duplicateError.message)
        assertFalse(duplicateError.message.orEmpty().contains(testFile().absolutePath))

        first.closeAndAwait()
        activeStore = null
        val reopened = openStore()
        assertEquals(CredentialRecord(), reopened.read())
    }

    private fun openStore(): CredentialDataStore =
        CredentialDataStore.createForTesting(context, fileName).also { activeStore = it }

    private fun repository(store: CredentialDataStore): AndroidStreamingCredentialRepository =
        AndroidStreamingCredentialRepository(
            store = store,
            cipher = AndroidKeyStoreCredentialCipher(
                AndroidKeyStoreCredentialPolicy.production.copy(alias = alias),
            ),
        )

    private fun testFile(): File = File(context.noBackupFilesDir, fileName)

    private fun keyStore(): KeyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    private fun deleteAlias(value: String) {
        keyStore().run {
            if (containsAlias(value)) {
                deleteEntry(value)
            }
        }
    }
}
