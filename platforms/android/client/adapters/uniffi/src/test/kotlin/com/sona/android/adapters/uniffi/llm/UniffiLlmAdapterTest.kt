package com.sona.android.adapters.uniffi.llm

import com.sona.android.application.llm.LlmConfig
import com.sona.android.application.llm.LlmFailureCategory
import com.sona.android.application.llm.LlmSummaryTemplate
import com.sona.android.application.llm.LlmTaskException
import com.sona.android.application.llm.LlmTaskKind
import com.sona.android.application.llm.LlmTaskProgress
import com.sona.android.application.llm.LlmTaskState
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiLlmTaskChunk
import uniffi.sona_uniffi_bind.FfiLlmTaskFinal
import uniffi.sona_uniffi_bind.FfiLlmTaskObserver
import uniffi.sona_uniffi_bind.FfiLlmTaskProgress
import uniffi.sona_uniffi_bind.FfiLlmTaskText
import uniffi.sona_uniffi_bind.FfiLlmTaskType
import uniffi.sona_uniffi_bind.FfiPolishSegmentsRequest
import uniffi.sona_uniffi_bind.FfiSecret
import uniffi.sona_uniffi_bind.FfiSummarizeTranscriptRequest
import uniffi.sona_uniffi_bind.FfiTranslateSegmentsRequest
import uniffi.sona_uniffi_bind.NoHandle
import uniffi.sona_uniffi_bind.SonaCoreBindingException

class UniffiLlmAdapterTest {

    private class FakeUniffiLlmBindings : UniffiLlmBindings {
        var summaryRequest: FfiSummarizeTranscriptRequest? = null
        var translateRequest: FfiTranslateSegmentsRequest? = null
        var polishRequest: FfiPolishSegmentsRequest? = null

        var summaryResponse: (suspend (FfiLlmTaskObserver) -> FfiLlmTaskFinal)? = null
        var translateResponse: (suspend (FfiLlmTaskObserver) -> FfiLlmTaskFinal)? = null
        var polishResponse: (suspend (FfiLlmTaskObserver) -> FfiLlmTaskFinal)? = null

        override suspend fun runSummary(
            request: FfiSummarizeTranscriptRequest,
            observer: FfiLlmTaskObserver,
        ): FfiLlmTaskFinal {
            summaryRequest = request
            return summaryResponse?.invoke(observer)
                ?: FfiLlmTaskFinal(
                    taskId = request.taskId,
                    taskType = FfiLlmTaskType.SUMMARY,
                    resultJson = """{"content":"Default summary content","generatedAt":"2026-09-13T12:00:00Z","sourceFingerprint":"fp123"}""",
                )
        }

        override suspend fun runTranslate(
            request: FfiTranslateSegmentsRequest,
            observer: FfiLlmTaskObserver,
        ): FfiLlmTaskFinal {
            translateRequest = request
            return translateResponse?.invoke(observer)
                ?: FfiLlmTaskFinal(
                    taskId = request.taskId,
                    taskType = FfiLlmTaskType.TRANSLATE,
                    resultJson = """[{"id":"s1","translation":"Bonjour"}]""",
                )
        }

        override suspend fun runPolish(
            request: FfiPolishSegmentsRequest,
            observer: FfiLlmTaskObserver,
        ): FfiLlmTaskFinal {
            polishRequest = request
            return polishResponse?.invoke(observer)
                ?: FfiLlmTaskFinal(
                    taskId = request.taskId,
                    taskType = FfiLlmTaskType.POLISH,
                    resultJson = """[{"id":"s1","text":"Polished text"}]""",
                )
        }
    }

    private val sampleConfig = LlmConfig(
        providerId = "anthropic",
        strategy = "ANTHROPIC",
        baseUrl = "https://api.anthropic.com",
        model = "claude-3-haiku",
        configured = true,
    )

    private fun createAdapter(
        bindings: UniffiLlmBindings,
        config: LlmConfig = sampleConfig,
        apiKey: String = "sk-test",
    ) = UniffiLlmAdapter(config, apiKey, bindings) { FfiSecret(NoHandle) }

    private fun sampleSegment(
        id: String,
        text: String,
        startSeconds: Double = 0.0,
        endSeconds: Double = 1.0,
        isFinal: Boolean = true,
        translation: String? = null,
    ) = TranscriptSegment(
        id = id,
        text = text,
        startSeconds = startSeconds,
        endSeconds = endSeconds,
        isFinal = isFinal,
        translation = translation,
    )

    @Test
    fun `summarize passes request parameters and parses summary result`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        val recordedStates = mutableListOf<LlmTaskState>()
        bindings.summaryResponse = { observer ->
            observer.onProgress(FfiLlmTaskProgress("task-1", FfiLlmTaskType.SUMMARY, 1uL, 4uL))
            observer.onChunk(FfiLlmTaskChunk.Text("task-1", FfiLlmTaskType.SUMMARY, 0uL, 4uL, "Chunk summary text"))
            observer.onText(FfiLlmTaskText("task-1", FfiLlmTaskType.SUMMARY, "Full text update", "update", false))
            observer.onFinal(FfiLlmTaskFinal("task-1", FfiLlmTaskType.SUMMARY, ""))
            FfiLlmTaskFinal(
                taskId = "task-1",
                taskType = FfiLlmTaskType.SUMMARY,
                resultJson = """{"content":"Summary result content","generatedAt":"2026-09-13T15:30:00Z","sourceFingerprint":"sha_abc"}""",
            )
        }

        val adapter = createAdapter(bindings)
        val segments = listOf(
            sampleSegment("seg-1", "Hello world", 0.0, 2.0, true),
        )
        val template = LlmSummaryTemplate(id = "tpl-meeting", name = "Meeting Notes", instructions = "Format as bullet points")

        val result = adapter.summarize("rec-100", segments, template) { state ->
            recordedStates.add(state)
        }

        assertEquals("tpl-meeting", result.templateId)
        assertEquals("Summary result content", result.content)
        assertEquals("2026-09-13T15:30:00Z", result.generatedAt)
        assertEquals("sha_abc", result.sourceFingerprint)

        val req = checkNotNull(bindings.summaryRequest)
        assertEquals("android-summary-rec-100", req.taskId)
        assertEquals("anthropic", req.config.providerId)
        assertEquals("tpl-meeting", req.template.id)
        assertEquals("Format as bullet points", req.template.instructions)
        assertEquals(1, req.segments.size)
        assertEquals("seg-1", req.segments[0].id)

        assertEquals(4, recordedStates.size)
        assertEquals(LlmTaskState.Running(LlmTaskKind.SUMMARY, LlmTaskProgress(1, 4)), recordedStates[0])
        assertEquals(LlmTaskState.Running(LlmTaskKind.SUMMARY, LlmTaskProgress(1, 4), "Chunk summary text"), recordedStates[1])
        assertEquals(LlmTaskState.Running(LlmTaskKind.SUMMARY, LlmTaskProgress(0, 0), "Full text update"), recordedStates[2])
        assertEquals(LlmTaskState.Succeeded(LlmTaskKind.SUMMARY), recordedStates[3])
    }

    @Test
    fun `summarize falls back to generated values for generatedAt and sourceFingerprint`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        bindings.summaryResponse = {
            FfiLlmTaskFinal(
                taskId = "task-fallback",
                taskType = FfiLlmTaskType.SUMMARY,
                resultJson = """{"content":"Only content provided"}""",
            )
        }

        val adapter = createAdapter(bindings)
        val segments = listOf(
            sampleSegment("seg-1", "Source content for hash", 0.0, 1.0, true),
        )

        val result = adapter.summarize("rec-fallback", segments, LlmSummaryTemplate()) {}

        assertEquals("Only content provided", result.content)
        assertTrue(result.generatedAt.isNotBlank())
        assertTrue(result.sourceFingerprint.isNotBlank())
    }

    @Test
    fun `summarize maps runtime exception to category`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        bindings.summaryResponse = {
            throw SonaCoreBindingException.LlmRuntime("rate_limited", "Rate limit exceeded", 1000uL)
        }

        val adapter = createAdapter(bindings)
        val segments = listOf(sampleSegment("s1", "test"))

        try {
            adapter.summarize("rec-1", segments, LlmSummaryTemplate()) {}
            fail("Expected LlmTaskException")
        } catch (e: LlmTaskException) {
            assertEquals(LlmFailureCategory.RATE_LIMITED, e.category)
        }
    }

    @Test
    fun `summarize throws invalid response when content is blank or json malformed`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        val adapter = createAdapter(bindings)
        val segments = listOf(sampleSegment("s1", "test"))

        // Blank content
        bindings.summaryResponse = {
            FfiLlmTaskFinal("t", FfiLlmTaskType.SUMMARY, """{"content":"  "}""")
        }
        try {
            adapter.summarize("rec-1", segments, LlmSummaryTemplate()) {}
            fail("Expected LlmTaskException for blank content")
        } catch (e: LlmTaskException) {
            assertEquals(LlmFailureCategory.INVALID_RESPONSE, e.category)
        }

        // Malformed JSON
        bindings.summaryResponse = {
            FfiLlmTaskFinal("t", FfiLlmTaskType.SUMMARY, "not-a-json")
        }
        try {
            adapter.summarize("rec-1", segments, LlmSummaryTemplate()) {}
            fail("Expected LlmTaskException for malformed JSON")
        } catch (e: LlmTaskException) {
            assertEquals(LlmFailureCategory.INVALID_RESPONSE, e.category)
        }
    }

    @Test
    fun `translate updates segments with translated text and maps chunk items observer`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        val recordedStates = mutableListOf<LlmTaskState>()
        bindings.translateResponse = { observer ->
            observer.onChunk(FfiLlmTaskChunk.Items("t-tr", FfiLlmTaskType.TRANSLATE, 0uL, 1uL, """[{"id":"s1","translation":"Hola"}]"""))
            observer.onFinal(FfiLlmTaskFinal("t-tr", FfiLlmTaskType.TRANSLATE, ""))
            FfiLlmTaskFinal(
                taskId = "t-tr",
                taskType = FfiLlmTaskType.TRANSLATE,
                resultJson = """[{"id":"s1","translation":"Hola"},{"id":"s2","translation":"Mundo"}]""",
            )
        }

        val adapter = createAdapter(bindings)
        val segments = listOf(
            sampleSegment("s1", "Hello"),
            sampleSegment("s2", "World"),
        )

        val result = adapter.translate("rec-tr", segments, "es", "Spanish") { state ->
            recordedStates.add(state)
        }

        assertEquals(2, result.size)
        assertEquals("Hola", result[0].translation)
        assertEquals("Mundo", result[1].translation)

        val req = checkNotNull(bindings.translateRequest)
        assertEquals("android-translate-rec-tr", req.taskId)
        assertEquals("es", req.targetLanguage)
        assertEquals("Spanish", req.targetLanguageName)

        assertEquals(2, recordedStates.size)
        assertTrue(recordedStates[0] is LlmTaskState.Running)
        val runningState = recordedStates[0] as LlmTaskState.Running
        assertEquals(LlmTaskKind.TRANSLATE, runningState.kind)
        assertEquals("""[{"id":"s1","translation":"Hola"}]""", runningState.text)
        assertEquals(LlmTaskState.Succeeded(LlmTaskKind.TRANSLATE), recordedStates[1])
    }

    @Test
    fun `translate throws invalid response when a segment translation is missing or blank`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        val adapter = createAdapter(bindings)
        val segments = listOf(
            sampleSegment("s1", "Hello"),
            sampleSegment("s2", "World"),
        )

        // Missing segment s2
        bindings.translateResponse = {
            FfiLlmTaskFinal("t", FfiLlmTaskType.TRANSLATE, """[{"id":"s1","translation":"Hola"}]""")
        }
        try {
            adapter.translate("rec-1", segments, "es", null) {}
            fail("Expected LlmTaskException for missing segment")
        } catch (e: LlmTaskException) {
            assertEquals(LlmFailureCategory.INVALID_RESPONSE, e.category)
        }

        // Blank translation for s2
        bindings.translateResponse = {
            FfiLlmTaskFinal("t", FfiLlmTaskType.TRANSLATE, """[{"id":"s1","translation":"Hola"},{"id":"s2","translation":""}]""")
        }
        try {
            adapter.translate("rec-1", segments, "es", null) {}
            fail("Expected LlmTaskException for blank translation")
        } catch (e: LlmTaskException) {
            assertEquals(LlmFailureCategory.INVALID_RESPONSE, e.category)
        }
    }

    @Test
    fun `polish updates segment text with polished result`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        bindings.polishResponse = { observer ->
            observer.onFinal(FfiLlmTaskFinal("p1", FfiLlmTaskType.POLISH, ""))
            FfiLlmTaskFinal(
                taskId = "p1",
                taskType = FfiLlmTaskType.POLISH,
                resultJson = """[{"id":"s1","text":"Polished sentence one."}]""",
            )
        }

        val adapter = createAdapter(bindings)
        val segments = listOf(sampleSegment("s1", "raw text"))

        val result = adapter.polish("rec-pol", segments) {}

        assertEquals(1, result.size)
        assertEquals("Polished sentence one.", result[0].text)

        val req = checkNotNull(bindings.polishRequest)
        assertEquals("android-polish-rec-pol", req.taskId)
    }

    @Test
    fun `polish throws invalid response when polished text is blank`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        bindings.polishResponse = {
            FfiLlmTaskFinal("p1", FfiLlmTaskType.POLISH, """[{"id":"s1","text":""}]""")
        }

        val adapter = createAdapter(bindings)
        val segments = listOf(sampleSegment("s1", "raw"))

        try {
            adapter.polish("rec-1", segments) {}
            fail("Expected LlmTaskException for blank polished text")
        } catch (e: LlmTaskException) {
            assertEquals(LlmFailureCategory.INVALID_RESPONSE, e.category)
        }
    }

    @Test
    fun `callLlm propagates CancellationException directly`() = runTest {
        val bindings = FakeUniffiLlmBindings()
        bindings.summaryResponse = {
            throw CancellationException("Job was cancelled")
        }

        val adapter = createAdapter(bindings)
        try {
            adapter.summarize("rec-1", listOf(sampleSegment("s1", "test")), LlmSummaryTemplate()) {}
            fail("Expected CancellationException")
        } catch (e: CancellationException) {
            assertEquals("Job was cancelled", e.message)
        }
    }

    @Test
    fun `mapLlmFailure maps SonaCoreBindingException codes correctly`() {
        assertEquals(
            LlmFailureCategory.AUTHENTICATION,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("authentication", "", null)),
        )
        assertEquals(
            LlmFailureCategory.AUTHENTICATION,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("unauthorized", "", null)),
        )
        assertEquals(
            LlmFailureCategory.AUTHENTICATION,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("invalid_api_key", "", null)),
        )
        assertEquals(
            LlmFailureCategory.RATE_LIMITED,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("rate_limited", "", null)),
        )
        assertEquals(
            LlmFailureCategory.RATE_LIMITED,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("rate_limit", "", null)),
        )
        assertEquals(
            LlmFailureCategory.NETWORK,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("network", "", null)),
        )
        assertEquals(
            LlmFailureCategory.NETWORK,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("timeout", "", null)),
        )
        assertEquals(
            LlmFailureCategory.NETWORK,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("provider_unavailable", "", null)),
        )
        assertEquals(
            LlmFailureCategory.INVALID_RESPONSE,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("invalid_response", "", null)),
        )
        assertEquals(
            LlmFailureCategory.INVALID_RESPONSE,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("incomplete", "", null)),
        )
        assertEquals(
            LlmFailureCategory.UNSUPPORTED,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("unsupported", "", null)),
        )
        assertEquals(
            LlmFailureCategory.NOT_CONFIGURED,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("not_configured", "", null)),
        )
        assertEquals(
            LlmFailureCategory.UNKNOWN,
            mapLlmFailure(SonaCoreBindingException.LlmRuntime("unexpected_xyz", "", null)),
        )
    }

    @Test
    fun `mapLlmFailure falls back to message keywords for generic exceptions`() {
        assertEquals(
            LlmFailureCategory.AUTHENTICATION,
            mapLlmFailure(RuntimeException("Invalid API key provided")),
        )
        assertEquals(
            LlmFailureCategory.RATE_LIMITED,
            mapLlmFailure(RuntimeException("HTTP 429 Too Many Requests")),
        )
        assertEquals(
            LlmFailureCategory.NETWORK,
            mapLlmFailure(RuntimeException("Connection timeout occurred")),
        )
        assertEquals(
            LlmFailureCategory.UNSUPPORTED,
            mapLlmFailure(RuntimeException("Feature is unsupported by model")),
        )
        assertEquals(
            LlmFailureCategory.INVALID_RESPONSE,
            mapLlmFailure(RuntimeException("Failed to parse json body")),
        )
        assertEquals(
            LlmFailureCategory.UNKNOWN,
            mapLlmFailure(RuntimeException("Something unusual happened")),
        )
    }

    @Test
    fun `resolveLlmStrategy resolves known providers and fallback strategies`() {
        assertEquals("ANTHROPIC", resolveLlmStrategy("anthropic"))
        assertEquals("OLLAMA", resolveLlmStrategy("ollama"))
        assertEquals("GEMINI", resolveLlmStrategy("gemini"))
        assertEquals("AZURE_OPEN_AI", resolveLlmStrategy("azure_openai"))
        assertEquals("AZURE_OPEN_AI", resolveLlmStrategy("azure_open_ai"))
        assertEquals("OPEN_AI_RESPONSES", resolveLlmStrategy("open_ai_responses"))
        assertEquals("OPEN_AI_RESPONSES", resolveLlmStrategy("openai_responses"))
        assertEquals("PERPLEXITY", resolveLlmStrategy("perplexity"))
        assertEquals("COPILOT", resolveLlmStrategy("copilot"))
        assertEquals("GOOGLE_TRANSLATE", resolveLlmStrategy("google_translate"))
        assertEquals("GOOGLE_TRANSLATE_FREE", resolveLlmStrategy("google_translate_free"))
        assertEquals("OPEN_AI_COMPATIBLE_CUSTOM_PATH", resolveLlmStrategy("open_ai_compatible_custom_path"))
        assertEquals("OPEN_AI_COMPATIBLE", resolveLlmStrategy("custom_unknown_provider"))
    }
}
