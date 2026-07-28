package com.sona.android.adapters.android.settings

import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalSherpaConfig
import com.sona.android.application.recording.OnlineAsrProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RecognitionSettingsMigrationTest {
    @Test
    fun `online migration keeps volcengine for live and old provider for batch`() {
        val migrated = migrateLegacyRecognitionSettings(
            legacyEngine = "ONLINE",
            legacyModelId = null,
            installedModels = emptyList(),
            legacyBatchProvider = OnlineAsrProvider.GROQ_WHISPER,
        )

        assertEquals(
            AsrModelSelection.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO),
            migrated.liveSelection,
        )
        assertEquals(
            AsrModelSelection.Online(OnlineAsrProvider.GROQ_WHISPER),
            migrated.batchSelection,
        )
    }

    @Test
    fun `dual mode local model populates both slots`() {
        val model = model(setOf(AsrMode.STREAMING, AsrMode.BATCH))

        val migrated = migrateLegacyRecognitionSettings(
            legacyEngine = "LOCAL",
            legacyModelId = model.id,
            installedModels = listOf(model),
            legacyBatchProvider = OnlineAsrProvider.MISTRAL_VOXTRAL,
        )

        assertEquals(AsrModelSelection.Local(model.id), migrated.liveSelection)
        assertEquals(AsrModelSelection.Local(model.id), migrated.batchSelection)
    }

    @Test
    fun `streaming only local model leaves batch unconfigured`() {
        val model = model(setOf(AsrMode.STREAMING))

        val migrated = migrateLegacyRecognitionSettings(
            legacyEngine = "LOCAL",
            legacyModelId = model.id,
            installedModels = listOf(model),
            legacyBatchProvider = OnlineAsrProvider.GROQ_WHISPER,
        )

        assertEquals(AsrModelSelection.Local(model.id), migrated.liveSelection)
        assertNull(migrated.batchSelection)
    }

    private fun model(modes: Set<AsrMode>) = LocalAsrModel(
        id = "legacy-model",
        displayName = "Legacy model",
        config = LocalSherpaConfig("/models/legacy", 2, "sensevoice"),
        supportedModes = modes,
    )
}
