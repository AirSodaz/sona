use sona_core::llm::providers::{LlmProvider, LlmProviderDefaults};
use sona_core::llm::requests::{
    LlmConfig, PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, PolishedSegment, SummarySegmentInput,
    SummaryTemplateConfig, TranslatedSegment,
};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmProviderDefaults {
    pub api_host: String,
    pub api_path: Option<String>,
    pub api_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmProvider {
    pub id: String,
    pub aliases: Vec<String>,
    pub defaults: FfiLlmProviderDefaults,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmProviderStrategy {
    OpenAi,
    OpenAiResponses,
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
    GoogleTranslate,
    GoogleTranslateFree,
    OpenAiCompatible,
    OpenAiCompatibleCustomPath,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmConfig {
    pub provider_id: String,
    pub strategy: FfiLlmProviderStrategy,
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

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmSegmentInput {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSummarySegmentInput {
    pub id: String,
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub is_final: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmPromptChunk {
    pub start: u64,
    pub end: u64,
    pub chunk_number: u64,
    pub total_chunks: u64,
    pub prompt: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiPolishedSegment {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTranslatedSegment {
    pub id: String,
    pub translation: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiPolishSegmentsRequest {
    pub task_id: String,
    pub config: FfiLlmConfig,
    pub segments: Vec<FfiLlmSegmentInput>,
    pub chunk_size: Option<u64>,
    pub context: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranslateSegmentsRequest {
    pub task_id: String,
    pub config: FfiLlmConfig,
    pub segments: Vec<FfiLlmSegmentInput>,
    pub chunk_size: Option<u64>,
    pub target_language: String,
    pub target_language_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSummarizeTranscriptRequest {
    pub task_id: String,
    pub config: FfiLlmConfig,
    pub template: FfiSummaryTemplateConfig,
    pub segments: Vec<FfiSummarySegmentInput>,
    pub chunk_char_budget: Option<u64>,
}

fn llm_provider_defaults_to_ffi(defaults: &LlmProviderDefaults) -> FfiLlmProviderDefaults {
    FfiLlmProviderDefaults {
        api_host: defaults.api_host.clone(),
        api_path: defaults.api_path.clone(),
        api_version: defaults.api_version.clone(),
    }
}

pub fn llm_provider_to_ffi(provider: &LlmProvider) -> FfiLlmProvider {
    FfiLlmProvider {
        id: provider.id.clone(),
        aliases: provider.aliases.clone(),
        defaults: llm_provider_defaults_to_ffi(&provider.defaults),
    }
}

pub fn llm_provider_strategy_to_ffi(strategy: LlmProviderStrategy) -> FfiLlmProviderStrategy {
    match strategy {
        LlmProviderStrategy::OpenAi => FfiLlmProviderStrategy::OpenAi,
        LlmProviderStrategy::OpenAiResponses => FfiLlmProviderStrategy::OpenAiResponses,
        LlmProviderStrategy::AzureOpenAi => FfiLlmProviderStrategy::AzureOpenAi,
        LlmProviderStrategy::Anthropic => FfiLlmProviderStrategy::Anthropic,
        LlmProviderStrategy::Gemini => FfiLlmProviderStrategy::Gemini,
        LlmProviderStrategy::Ollama => FfiLlmProviderStrategy::Ollama,
        LlmProviderStrategy::DeepSeek => FfiLlmProviderStrategy::DeepSeek,
        LlmProviderStrategy::MoonshotAi => FfiLlmProviderStrategy::MoonshotAi,
        LlmProviderStrategy::MoonshotCn => FfiLlmProviderStrategy::MoonshotCn,
        LlmProviderStrategy::Xiaomi => FfiLlmProviderStrategy::Xiaomi,
        LlmProviderStrategy::Kimi => FfiLlmProviderStrategy::Kimi,
        LlmProviderStrategy::SiliconFlow => FfiLlmProviderStrategy::SiliconFlow,
        LlmProviderStrategy::Qwen => FfiLlmProviderStrategy::Qwen,
        LlmProviderStrategy::QwenPortal => FfiLlmProviderStrategy::QwenPortal,
        LlmProviderStrategy::MinimaxGlobal => FfiLlmProviderStrategy::MinimaxGlobal,
        LlmProviderStrategy::MinimaxCn => FfiLlmProviderStrategy::MinimaxCn,
        LlmProviderStrategy::OpenRouter => FfiLlmProviderStrategy::OpenRouter,
        LlmProviderStrategy::LmStudio => FfiLlmProviderStrategy::LmStudio,
        LlmProviderStrategy::Groq => FfiLlmProviderStrategy::Groq,
        LlmProviderStrategy::XAi => FfiLlmProviderStrategy::XAi,
        LlmProviderStrategy::MistralAi => FfiLlmProviderStrategy::MistralAi,
        LlmProviderStrategy::Perplexity => FfiLlmProviderStrategy::Perplexity,
        LlmProviderStrategy::Volcengine => FfiLlmProviderStrategy::Volcengine,
        LlmProviderStrategy::Chatglm => FfiLlmProviderStrategy::Chatglm,
        LlmProviderStrategy::Copilot => FfiLlmProviderStrategy::Copilot,
        LlmProviderStrategy::GoogleTranslate => FfiLlmProviderStrategy::GoogleTranslate,
        LlmProviderStrategy::GoogleTranslateFree => FfiLlmProviderStrategy::GoogleTranslateFree,
        LlmProviderStrategy::OpenAiCompatible => FfiLlmProviderStrategy::OpenAiCompatible,
        LlmProviderStrategy::OpenAiCompatibleCustomPath => {
            FfiLlmProviderStrategy::OpenAiCompatibleCustomPath
        }
    }
}

pub fn llm_config_to_ffi(config: LlmConfig) -> FfiLlmConfig {
    FfiLlmConfig {
        provider_id: config.provider.as_str(),
        strategy: llm_provider_strategy_to_ffi(config.strategy),
        base_url: config.base_url,
        api_key: config.api_key,
        model: config.model,
        api_path: config.api_path,
        api_version: config.api_version,
        temperature: config.temperature,
        reasoning_enabled: config.reasoning_enabled,
        reasoning_level: config.reasoning_level,
        timeout_seconds: config.timeout_seconds,
    }
}

pub fn llm_segment_input_to_ffi(segment: LlmSegmentInput) -> FfiLlmSegmentInput {
    FfiLlmSegmentInput {
        id: segment.id,
        text: segment.text,
    }
}

pub fn summary_segment_input_to_ffi(segment: SummarySegmentInput) -> FfiSummarySegmentInput {
    FfiSummarySegmentInput {
        id: segment.id,
        text: segment.text,
        start: segment.start,
        end: segment.end,
        is_final: segment.is_final,
    }
}

fn summary_template_config_to_ffi(template: SummaryTemplateConfig) -> FfiSummaryTemplateConfig {
    FfiSummaryTemplateConfig {
        id: template.id,
        name: template.name,
        instructions: template.instructions,
    }
}

pub fn llm_prompt_chunk_to_ffi(
    start: usize,
    end: usize,
    chunk_number: usize,
    total_chunks: usize,
    prompt: String,
) -> FfiLlmPromptChunk {
    FfiLlmPromptChunk {
        start: start as u64,
        end: end as u64,
        chunk_number: chunk_number as u64,
        total_chunks: total_chunks as u64,
        prompt,
    }
}

pub fn polished_segment_to_ffi(segment: PolishedSegment) -> FfiPolishedSegment {
    FfiPolishedSegment {
        id: segment.id,
        text: segment.text,
    }
}

pub fn translated_segment_to_ffi(segment: TranslatedSegment) -> FfiTranslatedSegment {
    FfiTranslatedSegment {
        id: segment.id,
        translation: segment.translation,
    }
}

pub fn polish_segments_request_to_ffi(request: PolishSegmentsRequest) -> FfiPolishSegmentsRequest {
    FfiPolishSegmentsRequest {
        task_id: request.task_id,
        config: llm_config_to_ffi(request.config),
        segments: request
            .segments
            .into_iter()
            .map(llm_segment_input_to_ffi)
            .collect(),
        chunk_size: request.chunk_size.map(|value| value as u64),
        context: request.context,
        keywords: request.keywords,
    }
}

pub fn translate_segments_request_to_ffi(
    request: TranslateSegmentsRequest,
) -> FfiTranslateSegmentsRequest {
    FfiTranslateSegmentsRequest {
        task_id: request.task_id,
        config: llm_config_to_ffi(request.config),
        segments: request
            .segments
            .into_iter()
            .map(llm_segment_input_to_ffi)
            .collect(),
        chunk_size: request.chunk_size.map(|value| value as u64),
        target_language: request.target_language,
        target_language_name: request.target_language_name,
    }
}

pub fn summarize_transcript_request_to_ffi(
    request: SummarizeTranscriptRequest,
) -> FfiSummarizeTranscriptRequest {
    FfiSummarizeTranscriptRequest {
        task_id: request.task_id,
        config: llm_config_to_ffi(request.config),
        template: summary_template_config_to_ffi(request.template),
        segments: request
            .segments
            .into_iter()
            .map(summary_segment_input_to_ffi)
            .collect(),
        chunk_char_budget: request.chunk_char_budget.map(|value| value as u64),
    }
}
