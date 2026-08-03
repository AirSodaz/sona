use crate::FfiSecret;
use crate::SonaCoreBindingResult;
use sona_core::llm::providers::{LlmProvider, LlmProviderDefaults};
use sona_core::llm::requests::{
    LlmConfig, LlmGenerateRequest, LlmModelsRequest, PolishSegmentsRequest,
    SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, PolishedSegment, SummarySegmentInput,
    SummaryTemplateConfig, TranslatedSegment,
};
use sona_core::llm::usage::LlmGenerateSource;
use std::sync::Arc;

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

/// `api_key` is an opaque handle so the generated Kotlin `data class`
/// `toString()` cannot print it.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmConfig {
    pub provider_id: String,
    pub strategy: FfiLlmProviderStrategy,
    pub base_url: String,
    pub api_key: Arc<FfiSecret>,
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
        api_key: FfiSecret::new(config.api_key),
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

// From-FFI conversions for the typed V1 LLM surface. The `_json` functions keep
// parsing their payloads; these let the `_v1` twins take the records directly.

pub(crate) fn llm_segment_input_from_ffi(segment: FfiLlmSegmentInput) -> LlmSegmentInput {
    LlmSegmentInput {
        id: segment.id,
        text: segment.text,
    }
}

pub(crate) fn summary_segment_input_from_ffi(
    segment: FfiSummarySegmentInput,
) -> SummarySegmentInput {
    SummarySegmentInput {
        id: segment.id,
        text: segment.text,
        start: segment.start,
        end: segment.end,
        is_final: segment.is_final,
    }
}

pub(crate) fn summary_template_config_from_ffi(
    template: FfiSummaryTemplateConfig,
) -> SummaryTemplateConfig {
    SummaryTemplateConfig {
        id: template.id,
        name: template.name,
        instructions: template.instructions,
    }
}

pub(crate) fn polished_segment_from_ffi(segment: FfiPolishedSegment) -> PolishedSegment {
    PolishedSegment {
        id: segment.id,
        text: segment.text,
    }
}

pub(crate) fn translated_segment_from_ffi(segment: FfiTranslatedSegment) -> TranslatedSegment {
    TranslatedSegment {
        id: segment.id,
        translation: segment.translation,
    }
}

/// `provider_id` round-trips losslessly: `LlmProvider::as_str` produced it and
/// `IntoLlmProvider` resolves it back, falling back to `Custom` for unknown ids
/// exactly as JSON deserialization does.
pub(crate) fn llm_config_from_ffi(config: FfiLlmConfig) -> LlmConfig {
    LlmConfig {
        provider: sona_core::domain::IntoLlmProvider::into_provider(config.provider_id),
        strategy: llm_provider_strategy_from_ffi(config.strategy),
        base_url: config.base_url,
        api_key: config.api_key.expose().to_string(),
        model: config.model,
        api_path: config.api_path,
        api_version: config.api_version,
        temperature: config.temperature,
        reasoning_enabled: config.reasoning_enabled,
        reasoning_level: config.reasoning_level,
        timeout_seconds: config.timeout_seconds,
    }
}

fn llm_provider_strategy_from_ffi(strategy: FfiLlmProviderStrategy) -> LlmProviderStrategy {
    match strategy {
        FfiLlmProviderStrategy::OpenAi => LlmProviderStrategy::OpenAi,
        FfiLlmProviderStrategy::OpenAiResponses => LlmProviderStrategy::OpenAiResponses,
        FfiLlmProviderStrategy::AzureOpenAi => LlmProviderStrategy::AzureOpenAi,
        FfiLlmProviderStrategy::Anthropic => LlmProviderStrategy::Anthropic,
        FfiLlmProviderStrategy::Gemini => LlmProviderStrategy::Gemini,
        FfiLlmProviderStrategy::Ollama => LlmProviderStrategy::Ollama,
        FfiLlmProviderStrategy::DeepSeek => LlmProviderStrategy::DeepSeek,
        FfiLlmProviderStrategy::MoonshotAi => LlmProviderStrategy::MoonshotAi,
        FfiLlmProviderStrategy::MoonshotCn => LlmProviderStrategy::MoonshotCn,
        FfiLlmProviderStrategy::Xiaomi => LlmProviderStrategy::Xiaomi,
        FfiLlmProviderStrategy::Kimi => LlmProviderStrategy::Kimi,
        FfiLlmProviderStrategy::SiliconFlow => LlmProviderStrategy::SiliconFlow,
        FfiLlmProviderStrategy::Qwen => LlmProviderStrategy::Qwen,
        FfiLlmProviderStrategy::QwenPortal => LlmProviderStrategy::QwenPortal,
        FfiLlmProviderStrategy::MinimaxGlobal => LlmProviderStrategy::MinimaxGlobal,
        FfiLlmProviderStrategy::MinimaxCn => LlmProviderStrategy::MinimaxCn,
        FfiLlmProviderStrategy::OpenRouter => LlmProviderStrategy::OpenRouter,
        FfiLlmProviderStrategy::LmStudio => LlmProviderStrategy::LmStudio,
        FfiLlmProviderStrategy::Groq => LlmProviderStrategy::Groq,
        FfiLlmProviderStrategy::XAi => LlmProviderStrategy::XAi,
        FfiLlmProviderStrategy::MistralAi => LlmProviderStrategy::MistralAi,
        FfiLlmProviderStrategy::Perplexity => LlmProviderStrategy::Perplexity,
        FfiLlmProviderStrategy::Volcengine => LlmProviderStrategy::Volcengine,
        FfiLlmProviderStrategy::Chatglm => LlmProviderStrategy::Chatglm,
        FfiLlmProviderStrategy::Copilot => LlmProviderStrategy::Copilot,
        FfiLlmProviderStrategy::GoogleTranslate => LlmProviderStrategy::GoogleTranslate,
        FfiLlmProviderStrategy::GoogleTranslateFree => LlmProviderStrategy::GoogleTranslateFree,
        FfiLlmProviderStrategy::OpenAiCompatible => LlmProviderStrategy::OpenAiCompatible,
        FfiLlmProviderStrategy::OpenAiCompatibleCustomPath => {
            LlmProviderStrategy::OpenAiCompatibleCustomPath
        }
    }
}

pub(crate) fn polish_segments_request_from_ffi(
    request: FfiPolishSegmentsRequest,
) -> SonaCoreBindingResult<PolishSegmentsRequest> {
    Ok(PolishSegmentsRequest {
        task_id: request.task_id,
        config: llm_config_from_ffi(request.config),
        segments: request
            .segments
            .into_iter()
            .map(llm_segment_input_from_ffi)
            .collect(),
        chunk_size: optional_u64_to_usize(request.chunk_size, "chunk size")?,
        context: request.context,
        keywords: request.keywords,
    })
}

pub(crate) fn translate_segments_request_from_ffi(
    request: FfiTranslateSegmentsRequest,
) -> SonaCoreBindingResult<TranslateSegmentsRequest> {
    Ok(TranslateSegmentsRequest {
        task_id: request.task_id,
        config: llm_config_from_ffi(request.config),
        segments: request
            .segments
            .into_iter()
            .map(llm_segment_input_from_ffi)
            .collect(),
        chunk_size: optional_u64_to_usize(request.chunk_size, "chunk size")?,
        target_language: request.target_language,
        target_language_name: request.target_language_name,
    })
}

pub(crate) fn summarize_transcript_request_from_ffi(
    request: FfiSummarizeTranscriptRequest,
) -> SonaCoreBindingResult<SummarizeTranscriptRequest> {
    Ok(SummarizeTranscriptRequest {
        task_id: request.task_id,
        config: llm_config_from_ffi(request.config),
        template: summary_template_config_from_ffi(request.template),
        segments: request
            .segments
            .into_iter()
            .map(summary_segment_input_from_ffi)
            .collect(),
        chunk_char_budget: optional_u64_to_usize(request.chunk_char_budget, "chunk char budget")?,
    })
}

fn optional_u64_to_usize(value: Option<u64>, label: &str) -> SonaCoreBindingResult<Option<usize>> {
    value
        .map(|value| {
            usize::try_from(value).map_err(|_| crate::SonaCoreBindingError::InvalidInput {
                reason: format!("{label} is too large"),
            })
        })
        .transpose()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmGenerateSourceV1 {
    TitleGeneration,
    ConnectionTest,
    Generic,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmGenerateRequestV1 {
    pub config: FfiLlmConfig,
    pub input: String,
    pub source: Option<FfiLlmGenerateSourceV1>,
}

/// `api_key` is an opaque handle for the same reason as `FfiLlmConfig`.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmModelsRequestV1 {
    pub provider_id: String,
    pub strategy: Option<FfiLlmProviderStrategy>,
    pub base_url: String,
    pub api_key: Arc<FfiSecret>,
}

impl From<FfiLlmGenerateSourceV1> for LlmGenerateSource {
    fn from(value: FfiLlmGenerateSourceV1) -> Self {
        match value {
            FfiLlmGenerateSourceV1::TitleGeneration => Self::TitleGeneration,
            FfiLlmGenerateSourceV1::ConnectionTest => Self::ConnectionTest,
            FfiLlmGenerateSourceV1::Generic => Self::Generic,
        }
    }
}

pub(crate) fn llm_generate_request_from_ffi(
    request: FfiLlmGenerateRequestV1,
) -> LlmGenerateRequest {
    LlmGenerateRequest {
        config: llm_config_from_ffi(request.config),
        input: request.input,
        source: request.source.map(Into::into),
    }
}

pub(crate) fn llm_models_request_from_ffi(request: FfiLlmModelsRequestV1) -> LlmModelsRequest {
    LlmModelsRequest {
        provider: sona_core::domain::IntoLlmProvider::into_provider(request.provider_id),
        strategy: request.strategy.map(llm_provider_strategy_from_ffi),
        base_url: request.base_url,
        api_key: request.api_key.expose().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ffi_config() -> FfiLlmConfig {
        FfiLlmConfig {
            provider_id: "openai".to_string(),
            strategy: FfiLlmProviderStrategy::OpenAi,
            base_url: "https://api.example".to_string(),
            api_key: FfiSecret::new("sk-secret".to_string()),
            model: "gpt-test".to_string(),
            api_path: Some("/v1/chat".to_string()),
            api_version: None,
            temperature: Some(0.4),
            reasoning_enabled: Some(true),
            reasoning_level: Some("high".to_string()),
            timeout_seconds: Some(30),
        }
    }

    #[test]
    fn config_round_trips_through_core_without_losing_a_field() {
        let core = llm_config_from_ffi(ffi_config());
        let back = llm_config_to_ffi(core);

        // `provider_id` survives via `as_str`/`IntoLlmProvider`.
        assert_eq!(back.provider_id, "openai");
        assert_eq!(back.strategy, FfiLlmProviderStrategy::OpenAi);
        assert_eq!(back.base_url, "https://api.example");
        assert_eq!(back.model, "gpt-test");
        assert_eq!(back.api_path.as_deref(), Some("/v1/chat"));
        assert_eq!(back.api_version, None);
        assert_eq!(back.temperature, Some(0.4));
        assert_eq!(back.reasoning_enabled, Some(true));
        assert_eq!(back.reasoning_level.as_deref(), Some("high"));
        assert_eq!(back.timeout_seconds, Some(30));
        assert_eq!(back.api_key.expose(), "sk-secret");
    }

    #[test]
    fn an_unknown_provider_id_becomes_a_custom_provider_rather_than_an_error() {
        let mut config = ffi_config();
        config.provider_id = "my-self-hosted".to_string();

        let back = llm_config_to_ffi(llm_config_from_ffi(config));

        assert_eq!(back.provider_id, "my-self-hosted");
    }

    #[test]
    fn config_never_prints_its_api_key() {
        let rendered = format!("{:?}", ffi_config());

        assert!(!rendered.contains("sk-secret"), "secret leaked: {rendered}");
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn polish_request_round_trips_including_segments_and_chunk_size() {
        let request = FfiPolishSegmentsRequest {
            task_id: "task-1".to_string(),
            config: ffi_config(),
            segments: vec![FfiLlmSegmentInput {
                id: "s1".to_string(),
                text: "hello".to_string(),
            }],
            chunk_size: Some(8),
            context: Some("ctx".to_string()),
            keywords: None,
        };

        let core = polish_segments_request_from_ffi(request).unwrap();
        let back = polish_segments_request_to_ffi(core);

        assert_eq!(back.task_id, "task-1");
        assert_eq!(back.segments.len(), 1);
        assert_eq!(back.segments[0].text, "hello");
        assert_eq!(back.chunk_size, Some(8));
        assert_eq!(back.context.as_deref(), Some("ctx"));
        assert_eq!(back.keywords, None);
    }

    #[test]
    fn an_oversized_chunk_size_is_rejected_rather_than_truncated() {
        let request = FfiPolishSegmentsRequest {
            task_id: "task-1".to_string(),
            config: ffi_config(),
            segments: Vec::new(),
            chunk_size: Some(u64::MAX),
            context: None,
            keywords: None,
        };

        // Only meaningful where usize is narrower than u64.
        if usize::BITS < u64::BITS {
            assert!(polish_segments_request_from_ffi(request).is_err());
        } else {
            assert!(polish_segments_request_from_ffi(request).is_ok());
        }
    }

    #[test]
    fn every_provider_strategy_maps_both_ways() {
        // Guards the 29-arm inverse mapping against a missed variant.
        for strategy in [
            LlmProviderStrategy::OpenAi,
            LlmProviderStrategy::Anthropic,
            LlmProviderStrategy::Gemini,
            LlmProviderStrategy::Volcengine,
            LlmProviderStrategy::GoogleTranslateFree,
            LlmProviderStrategy::OpenAiCompatibleCustomPath,
        ] {
            let ffi = llm_provider_strategy_to_ffi(strategy);
            assert_eq!(llm_provider_strategy_from_ffi(ffi), strategy);
        }
    }
}
