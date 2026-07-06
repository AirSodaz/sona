use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderStrategy {
    OpenAi,
    OpenAiResponses,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    MoonshotAi,
    MoonshotCn,
    Xiaomi,
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
    Copilot,
    #[serde(rename = "google_translate")]
    GoogleTranslate,
    #[serde(rename = "google_translate_free")]
    GoogleTranslateFree,
    OpenAiCompatible,
    OpenAiCompatibleCustomPath,
}

impl LlmProviderStrategy {
    pub(crate) fn from_provider(provider: &LlmProvider) -> Self {
        use crate::core::domain::{BuiltinLlmProvider, LlmProvider};
        match provider {
            LlmProvider::Custom(_) => Self::OpenAiCompatible,
            LlmProvider::Builtin(b) => match b {
                BuiltinLlmProvider::OpenAi => Self::OpenAi,
                BuiltinLlmProvider::OpenAiResponses => Self::OpenAiResponses,
                BuiltinLlmProvider::AzureOpenai => Self::AzureOpenAi,
                BuiltinLlmProvider::Anthropic => Self::Anthropic,
                BuiltinLlmProvider::Gemini => Self::Gemini,
                BuiltinLlmProvider::Ollama => Self::Ollama,
                BuiltinLlmProvider::MoonshotAi => Self::MoonshotAi,
                BuiltinLlmProvider::MoonshotCn => Self::MoonshotCn,
                BuiltinLlmProvider::Xiaomi => Self::Xiaomi,
                BuiltinLlmProvider::Perplexity => Self::Perplexity,
                BuiltinLlmProvider::Copilot => Self::Copilot,
                BuiltinLlmProvider::Volcengine => Self::OpenAiCompatibleCustomPath,
                BuiltinLlmProvider::GoogleTranslate => Self::GoogleTranslate,
                BuiltinLlmProvider::GoogleTranslateFree => Self::GoogleTranslateFree,
                _ => Self::OpenAiCompatible,
            },
        }
    }
}

impl<'de> Deserialize<'de> for LlmProviderStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "open_ai" => Self::OpenAi,
            "open_ai_responses" | "openai_responses" => Self::OpenAiResponses,
            "azure_openai" => Self::AzureOpenAi,
            "anthropic" => Self::Anthropic,
            "gemini" => Self::Gemini,
            "ollama" => Self::Ollama,
            "deep_seek" => Self::DeepSeek,
            "kimi" => Self::Kimi,
            "silicon_flow" => Self::SiliconFlow,
            "qwen" => Self::Qwen,
            "qwen_portal" => Self::QwenPortal,
            "minimax_global" => Self::MinimaxGlobal,
            "minimax_cn" => Self::MinimaxCn,
            "openrouter" | "open_router" => Self::OpenRouter,
            "lm_studio" => Self::LmStudio,
            "groq" => Self::Groq,
            "x_ai" => Self::XAi,
            "mistral_ai" => Self::MistralAi,
            "perplexity" => Self::Perplexity,
            "volcengine" => Self::Volcengine,
            "chatglm" => Self::Chatglm,
            "copilot" | "github_copilot" => Self::Copilot,
            "google_translate" => Self::GoogleTranslate,
            "google_translate_free" => Self::GoogleTranslateFree,
            "open_ai_compatible" | "openai_compatible" => Self::OpenAiCompatible,
            "open_ai_compatible_custom_path" | "openai_compatible_custom_path" => {
                Self::OpenAiCompatibleCustomPath
            }
            _ => Self::OpenAiCompatible,
        })
    }
}

pub use crate::core::domain::LlmProvider;
pub use sona_core::llm_usage::{LlmGenerateSource, LlmUsageCategory, TokenUsage};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskType {
    Polish,
    Translate,
    Summary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub strategy: LlmProviderStrategy,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub api_path: Option<String>,
    pub api_version: Option<String>,
    pub temperature: Option<f32>,
    pub reasoning_enabled: Option<bool>,
    pub reasoning_level: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLlmConfig {
    provider: LlmProvider,
    strategy: Option<LlmProviderStrategy>,
    base_url: String,
    api_key: String,
    model: String,
    api_path: Option<String>,
    api_version: Option<String>,
    temperature: Option<f32>,
    reasoning_enabled: Option<bool>,
    reasoning_level: Option<String>,
    timeout_seconds: Option<u64>,
}

impl<'de> Deserialize<'de> for LlmConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawLlmConfig::deserialize(deserializer)?;
        Ok(Self {
            strategy: raw
                .strategy
                .unwrap_or_else(|| LlmProviderStrategy::from_provider(&raw.provider)),
            provider: raw.provider,
            base_url: raw.base_url,
            api_key: raw.api_key,
            model: raw.model,
            api_path: raw.api_path,
            api_version: raw.api_version,
            temperature: raw.temperature,
            reasoning_enabled: raw.reasoning_enabled,
            reasoning_level: raw.reasoning_level,
            timeout_seconds: raw.timeout_seconds,
        })
    }
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
    #[serde(default)]
    pub strategy: Option<LlmProviderStrategy>,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelSummary {
    pub model: String,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_multimodal: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
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
    pub target_language_name: Option<String>, // Decoupled English name payload
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
    pub segments: Vec<crate::integrations::asr::TranscriptSegment>,
    pub target_language: Option<String>,
    pub target_language_name: Option<String>,
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
    pub segments: Option<Vec<crate::integrations::asr::TranscriptSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<HistorySummaryPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_item: Option<crate::repositories::history::HistoryItemRecord>,
}
