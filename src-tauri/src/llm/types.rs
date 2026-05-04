use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    OpenAi,
    OpenAiResponses,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    Kimi,
    SiliconFlow,
    Qwen,
    QwenPortal,
    MinimaxGlobal,
    MinimaxCn,
    OpenRouter,
    LmStudio,
    Groq,
    XAi,
    MistralAi,
    Perplexity,
    Volcengine,
    Chatglm,
    #[serde(rename = "google_translate")]
    GoogleTranslate,
    #[serde(rename = "google_translate_free")]
    GoogleTranslateFree,
    OpenAiCompatible,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskType {
    Polish,
    Translate,
    Summary,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmUsageCategory {
    Summary,
    Translation,
    Polish,
    TitleGeneration,
    ConnectionTest,
    Generic,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmGenerateSource {
    TitleGeneration,
    ConnectionTest,
    Generic,
}

impl From<LlmGenerateSource> for LlmUsageCategory {
    fn from(value: LlmGenerateSource) -> Self {
        match value {
            LlmGenerateSource::TitleGeneration => LlmUsageCategory::TitleGeneration,
            LlmGenerateSource::ConnectionTest => LlmUsageCategory::ConnectionTest,
            LlmGenerateSource::Generic => LlmUsageCategory::Generic,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub api_path: Option<String>,
    pub api_version: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmGenerateRequest {
    pub config: LlmConfig,
    pub input: String,
    pub source: Option<LlmGenerateSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StandardMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug)]
pub struct StandardLlmRequest {
    pub messages: Vec<StandardMessage>,
    pub temperature: f32,
    #[allow(dead_code)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StandardLlmResponse {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmUsageEventPayload {
    pub occurred_at: String,
    pub provider: LlmProvider,
    pub model: String,
    pub category: LlmUsageCategory,
    pub usage: Option<TokenUsage>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelsRequest {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmSegmentInput {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PolishSegmentsRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub segments: Vec<LlmSegmentInput>,
    pub chunk_size: Option<usize>,
    pub context: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TranslateSegmentsRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub segments: Vec<LlmSegmentInput>,
    pub chunk_size: Option<usize>,
    pub target_language: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummarySegmentInput {
    pub id: String,
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub is_final: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeTranscriptRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub template: SummaryTemplateConfig,
    pub segments: Vec<SummarySegmentInput>,
    pub chunk_char_budget: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolishedSegment {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSegment {
    pub id: String,
    pub translation: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryResult {
    pub template_id: String,
    pub content: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskProgressPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub completed_chunks: usize,
    pub total_chunks: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskChunkPayload<T> {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub items: Vec<T>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskTextPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub text: String,
    pub delta: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLlmJobRequest {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub job_history_id: Option<String>,
    pub config: LlmConfig,
    pub segments: Vec<crate::sherpa::TranscriptSegment>,
    pub target_language: Option<String>,
    pub context: Option<String>,
    pub keywords: Option<String>,
    pub template: Option<SummaryTemplateConfig>,
    pub chunk_size: Option<usize>,
    pub chunk_char_budget: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryRecordPayload {
    pub template_id: String,
    pub content: String,
    pub generated_at: String,
    pub source_fingerprint: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySummaryPayload {
    pub active_template_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<TranscriptSummaryRecordPayload>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLlmJobResult {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub job_history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<crate::sherpa::TranscriptSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<HistorySummaryPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_item: Option<crate::history_repository::HistoryItemRecord>,
}
