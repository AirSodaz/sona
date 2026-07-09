package com.sona.uniffi.sample

import uniffi.sona_uniffi_bind.FfiLlmPromptChunk
import uniffi.sona_uniffi_bind.FfiPolishedSegment
import uniffi.sona_uniffi_bind.SonaCoreBindingException
import uniffi.sona_uniffi_bind.defaultConfigJson
import uniffi.sona_uniffi_bind.parsePolishChunkJson
import uniffi.sona_uniffi_bind.planPolishPromptChunksJson

data class SonaUniffiSmokeResult(
    val defaultConfigJson: String,
    val chunks: List<FfiLlmPromptChunk>,
    val polished: List<FfiPolishedSegment>,
)

object SonaUniffiSmoke {
    private val sampleSegmentsJson = """
        [
          {"id":"s1","text":"hello from android"},
          {"id":"s2","text":"next mobile segment"}
        ]
    """.trimIndent()

    @Throws(SonaCoreBindingException::class)
    fun run(): SonaUniffiSmokeResult {
        val chunks = planPolishPromptChunksJson(
            segmentsJson = sampleSegmentsJson,
            context = "Android UniFFI smoke test",
            keywords = "Sona",
            chunkSize = 1UL,
            promptCharBudget = null,
        )
        val polished = parsePolishChunkJson(
            responseText = """
                {"id":"s1","text":"Hello from Android."}
            """.trimIndent(),
            expectedSegmentsJson = """
                [
                  {"id":"s1","text":"hello from android"}
                ]
            """.trimIndent(),
            chunkNumber = 1UL,
        )

        return SonaUniffiSmokeResult(
            defaultConfigJson = defaultConfigJson(),
            chunks = chunks,
            polished = polished,
        )
    }
}
