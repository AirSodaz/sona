package com.sona.android.adapters.android.llm

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import com.sona.android.adapters.android.credential.AndroidKeyStoreCredentialCipher
import com.sona.android.adapters.android.credential.AndroidKeyStoreCredentialPolicy
import com.sona.android.adapters.android.credential.CredentialEnvelope
import com.sona.android.adapters.android.credential.CredentialEnvelopeState
import com.sona.android.adapters.android.credential.CredentialRecord
import com.sona.android.application.llm.LlmConfig
import com.sona.android.application.llm.LlmConfigurationPort
import com.sona.android.application.llm.LlmProvider
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.flow.first

class AndroidLlmConfigurationRepository private constructor(
    private val dataStore: DataStore<androidx.datastore.preferences.core.Preferences>,
    private val cipher: AndroidKeyStoreCredentialCipher,
    private val providerLoader: () -> List<LlmProvider>,
) : LlmConfigurationPort {
    override val providers: Flow<List<LlmProvider>> = kotlinx.coroutines.flow.flow { emit(providerLoader()) }
    override val configuration: Flow<LlmConfig> = dataStore.data.map { preferences ->
        LlmConfig(
            providerId = preferences[PROVIDER] ?: DEFAULT_PROVIDER,
            strategy = preferences[STRATEGY] ?: DEFAULT_STRATEGY,
            baseUrl = preferences[BASE_URL] ?: "https://api.openai.com",
            model = preferences[MODEL] ?: "gpt-4o-mini",
            apiPath = preferences[API_PATH],
            apiVersion = preferences[API_VERSION],
            configured = preferences[CONFIGURED] == "true",
        )
    }

    override suspend fun save(config: LlmConfig, apiKey: String) {
        require(config.providerId.isNotBlank() && config.strategy.isNotBlank())
        require(config.baseUrl.startsWith("https://") || config.baseUrl.startsWith("http://"))
        require(config.model.isNotBlank() && apiKey.isNotBlank())
        val bytes = apiKey.encodeToByteArray()
        try {
            val envelope = cipher.encrypt(bytes).toRecord()
            dataStore.edit { preferences ->
                preferences[PROVIDER] = config.providerId
                preferences[STRATEGY] = config.strategy
                preferences[BASE_URL] = config.baseUrl.trim()
                preferences[MODEL] = config.model.trim()
                config.apiPath?.let { preferences[API_PATH] = it } ?: preferences.remove(API_PATH)
                config.apiVersion?.let { preferences[API_VERSION] = it } ?: preferences.remove(API_VERSION)
                preferences[CONFIGURED] = "true"
                preferences[KEY_FORMAT] = envelope.formatVersion.toString()
                preferences[KEY_IV] = envelope.ivBase64.orEmpty()
                preferences[KEY_CIPHERTEXT] = envelope.ciphertextBase64.orEmpty()
            }
        } finally {
            bytes.fill(0)
        }
    }

    override suspend fun loadApiKey(): String? {
        val preferences = dataStore.data.first()
        val record = CredentialRecord(
            formatVersion = preferences[KEY_FORMAT]?.toIntOrNull(),
            ivBase64 = preferences[KEY_IV],
            ciphertextBase64 = preferences[KEY_CIPHERTEXT],
        )
        val envelope = (CredentialEnvelope.inspect(record) as? CredentialEnvelopeState.Supported)?.envelope ?: return null
        val plaintext = cipher.decrypt(envelope)
        return try {
            Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(plaintext)).toString()
        } catch (_: Exception) { null } finally { plaintext.fill(0) }
    }

    override suspend fun clear() {
        dataStore.edit { preferences -> preferences.clear() }
        cipher.deleteKey()
    }

    companion object {
        private val PROVIDER = stringPreferencesKey("llm_provider")
        private val STRATEGY = stringPreferencesKey("llm_strategy")
        private val BASE_URL = stringPreferencesKey("llm_base_url")
        private val MODEL = stringPreferencesKey("llm_model")
        private val API_PATH = stringPreferencesKey("llm_api_path")
        private val API_VERSION = stringPreferencesKey("llm_api_version")
        private val CONFIGURED = stringPreferencesKey("llm_configured")
        private val KEY_FORMAT = stringPreferencesKey("llm_key_format")
        private val KEY_IV = stringPreferencesKey("llm_key_iv")
        private val KEY_CIPHERTEXT = stringPreferencesKey("llm_key_ciphertext")
        private const val DEFAULT_PROVIDER = "open_ai_compatible"
        private const val DEFAULT_STRATEGY = "OPEN_AI_COMPATIBLE"

        fun create(context: Context, providerLoader: () -> List<LlmProvider>): AndroidLlmConfigurationRepository {
            val appContext = context.applicationContext
            val store = PreferenceDataStoreFactory.create(
                corruptionHandler = androidx.datastore.core.handlers.ReplaceFileCorruptionHandler { emptyPreferences() },
                scope = CoroutineScope(Dispatchers.IO),
                produceFile = { appContext.noBackupFilesDir.resolve("llm_configuration.preferences_pb") },
            )
            val policy = AndroidKeyStoreCredentialPolicy.llm()
            return AndroidLlmConfigurationRepository(store, AndroidKeyStoreCredentialCipher(policy), providerLoader)
        }
    }
}
