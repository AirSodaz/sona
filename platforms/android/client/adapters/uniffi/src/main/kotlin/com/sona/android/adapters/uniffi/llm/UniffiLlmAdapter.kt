package com.sona.android.adapters.uniffi.llm

import com.sona.android.application.llm.LlmConfig
import com.sona.android.application.llm.LlmFailureCategory
import com.sona.android.application.llm.LlmProvider
import com.sona.android.application.llm.LlmSummary
import com.sona.android.application.llm.LlmSummaryTemplate
import com.sona.android.application.llm.LlmTaskObserver
import com.sona.android.application.llm.LlmTaskPort
import com.sona.android.application.llm.LlmTaskException
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import uniffi.sona_uniffi_bind.FfiLlmConfig
import uniffi.sona_uniffi_bind.FfiLlmProviderStrategy
import uniffi.sona_uniffi_bind.FfiLlmSegmentInput
import uniffi.sona_uniffi_bind.FfiLlmTaskObserver
import uniffi.sona_uniffi_bind.FfiLlmTaskProgress
import uniffi.sona_uniffi_bind.FfiLlmTaskText
import uniffi.sona_uniffi_bind.FfiPolishSegmentsRequest
import uniffi.sona_uniffi_bind.FfiSummarizeTranscriptRequest
import uniffi.sona_uniffi_bind.FfiSummarySegmentInput
import uniffi.sona_uniffi_bind.FfiSummaryTemplateConfig
import uniffi.sona_uniffi_bind.FfiTranslateSegmentsRequest
import uniffi.sona_uniffi_bind.FfiSecret
import uniffi.sona_uniffi_bind.FfiLlmTaskChunk
import uniffi.sona_uniffi_bind.FfiLlmTaskType
import uniffi.sona_uniffi_bind.llmProviders
import uniffi.sona_uniffi_bind.runLlmPolishV1
import uniffi.sona_uniffi_bind.runLlmSummaryV1
import uniffi.sona_uniffi_bind.runLlmTranslateV1
import uniffi.sona_uniffi_bind.SonaCoreBindingException
import java.security.MessageDigest
import java.time.Instant
import kotlinx.coroutines.CancellationException

class UniffiLlmAdapter(
    private val configuration: LlmConfig,
    private val apiKey: String,
) : LlmTaskPort {
    override suspend fun summarize(historyId: String, segments: List<TranscriptSegment>, template: LlmSummaryTemplate, observer: LlmTaskObserver): LlmSummary {
        val taskId = "android-summary-$historyId"
        val final = callLlm { runLlmSummaryV1(FfiSummarizeTranscriptRequest(taskId, config(), FfiSummaryTemplateConfig(template.id, template.name, template.instructions), segments.map { FfiSummarySegmentInput(it.id, it.text, it.startSeconds.toFloat(), it.endSeconds.toFloat(), it.isFinal) }, 1200uL), observer(taskId, observer)) }
        val value = runCatching { json.parseToJsonElement(final.resultJson).jsonObject }
            .getOrElse { throw LlmTaskException(mapLlmFailure(it), it) }
        val content = value["content"]?.jsonPrimitive?.content.orEmpty()
        if (content.isBlank()) throw LlmTaskException(LlmFailureCategory.INVALID_RESPONSE)
        val generatedAt = value["generatedAt"]?.jsonPrimitive?.content.orEmpty().ifBlank { Instant.now().toString() }
        val sourceFingerprint = value["sourceFingerprint"]?.jsonPrimitive?.content.orEmpty().ifBlank {
            sha256(segments.joinToString("\n") { "${it.id}:${it.text}" })
        }
        return LlmSummary(template.id, content, generatedAt, sourceFingerprint)
    }

    override suspend fun translate(historyId: String, segments: List<TranscriptSegment>, targetLanguage: String, targetLanguageName: String?, observer: LlmTaskObserver): List<TranscriptSegment> {
        val taskId = "android-translate-$historyId"
        val final = callLlm { runLlmTranslateV1(FfiTranslateSegmentsRequest(taskId, config(), segments.map { FfiLlmSegmentInput(it.id, it.text) }, 80uL, targetLanguage, targetLanguageName), observer(taskId, observer)) }
        val byId = runCatching { json.parseToJsonElement(final.resultJson).jsonArray.associate { it.jsonObject["id"]!!.jsonPrimitive.content to it.jsonObject["translation"]!!.jsonPrimitive.content } }
            .getOrElse { throw LlmTaskException(mapLlmFailure(it), it) }
        if (byId.size != segments.size || segments.any { it.id !in byId || byId[it.id].isNullOrBlank() }) throw LlmTaskException(LlmFailureCategory.INVALID_RESPONSE)
        return segments.map { it.copy(translation = byId[it.id]) }
    }

    override suspend fun polish(historyId: String, segments: List<TranscriptSegment>, observer: LlmTaskObserver): List<TranscriptSegment> {
        val taskId = "android-polish-$historyId"
        val final = callLlm { runLlmPolishV1(FfiPolishSegmentsRequest(taskId, config(), segments.map { FfiLlmSegmentInput(it.id, it.text) }, 80uL, null, null), observer(taskId, observer)) }
        val byId = runCatching { json.parseToJsonElement(final.resultJson).jsonArray.associate { it.jsonObject["id"]!!.jsonPrimitive.content to it.jsonObject["text"]!!.jsonPrimitive.content } }
            .getOrElse { throw LlmTaskException(mapLlmFailure(it), it) }
        if (byId.size != segments.size || segments.any { it.id !in byId || byId[it.id].isNullOrBlank() }) throw LlmTaskException(LlmFailureCategory.INVALID_RESPONSE)
        return segments.map { it.copy(text = byId[it.id].orEmpty()) }
    }

    private fun config() = FfiLlmConfig(configuration.providerId, strategy(configuration.strategy), configuration.baseUrl, FfiSecret(apiKey), configuration.model, configuration.apiPath, configuration.apiVersion, null, null, null, 60uL)

    private suspend fun <T> callLlm(block: suspend () -> T): T = try {
        block()
    } catch (error: CancellationException) {
        throw error
    } catch (error: LlmTaskException) {
        throw error
    } catch (error: Throwable) {
        throw LlmTaskException(mapLlmFailure(error), error)
    }

    private fun observer(taskId: String, delegate: LlmTaskObserver) = object : FfiLlmTaskObserver {
        override fun onProgress(event: FfiLlmTaskProgress) = delegate.onState(com.sona.android.application.llm.LlmTaskState.Running(event.taskType.toKind(), com.sona.android.application.llm.LlmTaskProgress(event.completedChunks.toLong(), event.totalChunks.toLong())))
        override fun onChunk(event: FfiLlmTaskChunk) = when (event) {
            is FfiLlmTaskChunk.Items -> delegate.onState(com.sona.android.application.llm.LlmTaskState.Running(event.taskType.toKind(), com.sona.android.application.llm.LlmTaskProgress(event.chunkIndex.toLong() + 1, event.totalChunks.toLong()), event.itemsJson))
            is FfiLlmTaskChunk.Text -> delegate.onState(com.sona.android.application.llm.LlmTaskState.Running(event.taskType.toKind(), com.sona.android.application.llm.LlmTaskProgress(event.chunkIndex.toLong() + 1, event.totalChunks.toLong()), event.text))
        }
        override fun onText(event: FfiLlmTaskText) = delegate.onState(com.sona.android.application.llm.LlmTaskState.Running(event.taskType.toKind(), com.sona.android.application.llm.LlmTaskProgress(0, 0), event.text))
        override fun onFinal(event: uniffi.sona_uniffi_bind.FfiLlmTaskFinal) = delegate.onState(com.sona.android.application.llm.LlmTaskState.Succeeded(event.taskType.toKind()))
    }

    private companion object {
        val json = Json { ignoreUnknownKeys = true }
        fun strategy(value: String): FfiLlmProviderStrategy = runCatching { FfiLlmProviderStrategy.valueOf(value.uppercase().replace('-', '_')) }.getOrElse { FfiLlmProviderStrategy.OPEN_AI_COMPATIBLE }
        fun FfiLlmTaskType.toKind() = when (this) { FfiLlmTaskType.POLISH -> com.sona.android.application.llm.LlmTaskKind.POLISH; FfiLlmTaskType.TRANSLATE -> com.sona.android.application.llm.LlmTaskKind.TRANSLATE; FfiLlmTaskType.SUMMARY -> com.sona.android.application.llm.LlmTaskKind.SUMMARY }
        fun sha256(value: String): String = MessageDigest.getInstance("SHA-256")
            .digest(value.toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }
    }
}

fun loadLlmProviders(): List<LlmProvider> = llmProviders().map { LlmProvider(it.id, it.aliases, it.defaults.apiHost, it.defaults.apiPath, it.defaults.apiVersion) }

fun mapLlmFailure(error: Throwable): LlmFailureCategory {
    if (error is SonaCoreBindingException.LlmRuntime) {
        return when (error.code.lowercase()) {
            "authentication", "unauthorized", "invalid_api_key" -> LlmFailureCategory.AUTHENTICATION
            "rate_limited", "rate_limit" -> LlmFailureCategory.RATE_LIMITED
            "network", "timeout", "provider_unavailable" -> LlmFailureCategory.NETWORK
            "invalid_response", "incomplete" -> LlmFailureCategory.INVALID_RESPONSE
            "unsupported" -> LlmFailureCategory.UNSUPPORTED
            "not_configured" -> LlmFailureCategory.NOT_CONFIGURED
            else -> LlmFailureCategory.UNKNOWN
        }
    }
    val message = error.message?.lowercase().orEmpty()
    return when {
        "authentication" in message || "unauthorized" in message || "api key" in message -> LlmFailureCategory.AUTHENTICATION
        "rate_limited" in message || "rate limited" in message || "429" in message -> LlmFailureCategory.RATE_LIMITED
        "network" in message || "timeout" in message || "unavailable" in message -> LlmFailureCategory.NETWORK
        "unsupported" in message -> LlmFailureCategory.UNSUPPORTED
        "invalid_response" in message || "incomplete" in message || "json" in message -> LlmFailureCategory.INVALID_RESPONSE
        else -> LlmFailureCategory.UNKNOWN
    }
}
