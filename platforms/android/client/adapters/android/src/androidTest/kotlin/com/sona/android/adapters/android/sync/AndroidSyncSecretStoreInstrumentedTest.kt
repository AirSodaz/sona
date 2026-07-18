package com.sona.android.adapters.android.sync

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.io.File
import java.security.KeyStore
import java.util.UUID
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AndroidSyncSecretStoreInstrumentedTest {
    private lateinit var context: Context
    private lateinit var fileName: String
    private var activeStore: AndroidSyncSecretStore? = null

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        fileName = "sync_secrets_test_${UUID.randomUUID()}.preferences_pb"
        testFile().delete()
    }

    @After
    fun tearDown() = runBlocking {
        activeStore?.closeAndAwait()
        activeStore = null
        testFile().delete()
    }

    @Test
    fun stores_webdav_password_and_vault_key_as_independent_secrets() = runBlocking {
        val store = openStore()
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"
        val password = "webdav-password".encodeToByteArray()
        val vaultKeyMaterial = byteArrayOf(1, 2, 3, 4)

        store.set(passwordKey, password)
        store.set(vaultKey, vaultKeyMaterial)

        assertArrayEquals(password, store.get(passwordKey))
        assertArrayEquals(vaultKeyMaterial, store.get(vaultKey))
        assertFalse(testFile().readText().contains("webdav-password"))

        password.fill(0)
        vaultKeyMaterial.fill(0)
    }

    @Test
    fun data_survives_reopening_the_store_with_the_same_android_keystore_key() = runBlocking {
        val key = "vault-key:vault-a"
        val expected = byteArrayOf(9, 8, 7)
        val first = openStore()
        first.set(key, expected)
        first.closeAndAwait()
        activeStore = null

        val reopened = openStore()

        assertArrayEquals(expected, reopened.get(key))
    }

    @Test
    fun invalidated_keystore_alias_clears_old_records_and_rotates_for_future_writes() = runBlocking {
        val store = openStore()
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"
        store.set(passwordKey, byteArrayOf(1))
        store.set(vaultKey, byteArrayOf(2))
        val policy = AndroidKeyStoreSyncSecretPolicy.production
        KeyStore.getInstance(policy.provider).apply {
            load(null)
            deleteEntry(policy.alias)
        }

        assertNull(store.get(passwordKey))
        assertNull(store.get(vaultKey))
        store.set(vaultKey, byteArrayOf(9, 8, 7))
        assertArrayEquals(byteArrayOf(9, 8, 7), store.get(vaultKey))
    }

    @Test
    fun delete_removes_only_the_requested_logical_key() = runBlocking {
        val store = openStore()
        val passwordKey = "webdav-password:vault-a"
        val vaultKey = "vault-key:vault-a"
        store.set(passwordKey, byteArrayOf(1))
        store.set(vaultKey, byteArrayOf(2))

        store.delete(passwordKey)

        assertNull(store.get(passwordKey))
        assertArrayEquals(byteArrayOf(2), store.get(vaultKey))
    }

    @Test
    fun default_storage_uses_the_dedicated_no_backup_sync_file() {
        assertEquals(
            File(context.noBackupFilesDir, "sync_secrets.preferences_pb").canonicalFile,
            AndroidSyncSecretStore.defaultFile(context).canonicalFile,
        )
    }

    @Test
    fun tampered_record_is_discarded_without_returning_a_secret() = runBlocking {
        val key = "vault-key:vault-a"
        val store = openStore()
        store.set(key, byteArrayOf(1, 2, 3))
        store.closeAndAwait()
        activeStore = null
        testFile().writeBytes(byteArrayOf(0x80.toByte()))

        val reopened = openStore()

        assertNull(reopened.get(key))
        assertTrue(testFile().exists())
    }

    private fun openStore(): AndroidSyncSecretStore =
        AndroidSyncSecretStore.createForTesting(context, fileName).also { activeStore = it }

    private fun testFile(): File = File(context.noBackupFilesDir, fileName)
}
