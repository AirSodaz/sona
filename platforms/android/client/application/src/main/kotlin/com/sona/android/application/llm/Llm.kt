package com.sona.android.application.llm

import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.flow.Flow

data class LlmProvider(
    val id: String,
    val aliases: List<String> = emptyList(),
    val apiHost: String = "",
    val apiPath: String? = null,
    val apiVersion: String? = null,
)

data class LlmConfig(
    val providerId: String = "open_ai_compatible",
    val strategy: String = "OPEN_AI_COMPATIBLE",
    val baseUrl: String = "https://api.openai.com",
    val model: String = "gpt-4o-mini",
    val apiPath: String? = null,
    val apiVersion: String? = null,
    val configured: Boolean = false,
)

data class LlmTaskProgress(val completedChunks: Long, val totalChunks: Long) {
    val percent: Int get() = if (totalChunks <= 0) 0 else (completedChunks * 100 / totalChunks).toInt().coerceIn(0, 100)
}

enum class LlmTaskKind { SUMMARY, TRANSLATE, POLISH }

sealed interface LlmTaskState {
    data object Idle : LlmTaskState
    data class Running(val kind: LlmTaskKind, val progress: LlmTaskProgress, val text: String = "") : LlmTaskState
    data class Succeeded(val kind: LlmTaskKind, val text: String = "") : LlmTaskState
    data class Failed(val kind: LlmTaskKind, val category: LlmFailureCategory) : LlmTaskState
}

enum class LlmFailureCategory { NOT_CONFIGURED, AUTHENTICATION, NETWORK, RATE_LIMITED, INVALID_RESPONSE, UNSUPPORTED, UNKNOWN }

data class LlmSummary(val templateId: String, val content: String, val generatedAt: String, val sourceFingerprint: String)

interface LlmConfigurationPort {
    val providers: Flow<List<LlmProvider>>
    val configuration: Flow<LlmConfig>
    suspend fun save(config: LlmConfig, apiKey: String)
    suspend fun loadApiKey(): String?
    suspend fun clear()
}

interface LlmTaskPort {
    suspend fun summarize(historyId: String, segments: List<TranscriptSegment>, template: LlmSummaryTemplate, observer: LlmTaskObserver): LlmSummary
    suspend fun translate(historyId: String, segments: List<TranscriptSegment>, targetLanguage: String, targetLanguageName: String?, observer: LlmTaskObserver): List<TranscriptSegment>
    suspend fun polish(historyId: String, segments: List<TranscriptSegment>, observer: LlmTaskObserver): List<TranscriptSegment>
}

data class LlmSummaryTemplate(val id: String = "general", val name: String = "General", val instructions: String = "Summarize the transcript clearly.")

fun interface LlmTaskObserver {
    fun onState(state: LlmTaskState)
}

interface LlmHistorySummaryPort {
    suspend fun loadSummary(historyId: String): LlmSummary?
    suspend fun saveSummary(historyId: String, summary: LlmSummary)
    suspend fun deleteSummary(historyId: String)
    suspend fun createSnapshot(historyId: String, reason: com.sona.android.application.library.TranscriptSnapshotReason)
    suspend fun commitTranscript(historyId: String, segments: List<TranscriptSegment>)
}
