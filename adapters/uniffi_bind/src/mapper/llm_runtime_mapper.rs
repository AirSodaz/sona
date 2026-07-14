use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};
use sona_core::llm::runtime::{LlmCompletionResponse, LlmExecutionMetadata, LlmResponseFormatKind};
use sona_core::llm::usage::TokenUsage;

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmResponseFormatKind {
    Text,
    JsonObject,
    JsonSchema,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmExecutionMetadata {
    pub requested_format: FfiLlmResponseFormatKind,
    pub applied_format: FfiLlmResponseFormatKind,
    pub warnings: Vec<String>,
    pub attempts: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmTokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmCompletionResponse {
    pub text: String,
    pub json: Option<String>,
    pub usage: Option<FfiLlmTokenUsage>,
    pub execution: FfiLlmExecutionMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmModality {
    Text,
    Image,
    Audio,
    Video,
    Pdf,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmModelMetadataSource {
    Provider,
    ModelsDev,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmModelSummary {
    pub model: String,
    pub display_name: Option<String>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    pub cache_read_price: Option<f64>,
    pub cache_write_price: Option<f64>,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub knowledge_cutoff: Option<String>,
    pub release_date: Option<String>,
    pub last_updated: Option<String>,
    pub input_modalities: Vec<FfiLlmModality>,
    pub output_modalities: Vec<FfiLlmModality>,
    pub supports_multimodal: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
    pub supports_structured_output: Option<bool>,
    pub supports_prompt_caching: Option<bool>,
    pub metadata_sources: Vec<FfiLlmModelMetadataSource>,
}

pub fn llm_completion_response_to_ffi(response: LlmCompletionResponse) -> FfiLlmCompletionResponse {
    FfiLlmCompletionResponse {
        text: response.text,
        json: response.json.map(|value| value.to_string()),
        usage: response.usage.map(llm_token_usage_to_ffi),
        execution: llm_execution_metadata_to_ffi(response.execution),
    }
}

pub fn llm_model_summary_to_ffi(model: LlmModelSummary) -> FfiLlmModelSummary {
    FfiLlmModelSummary {
        model: model.model,
        display_name: model.display_name,
        input_price: model.input_price,
        output_price: model.output_price,
        cache_read_price: model.cache_read_price,
        cache_write_price: model.cache_write_price,
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        knowledge_cutoff: model.knowledge_cutoff,
        release_date: model.release_date,
        last_updated: model.last_updated,
        input_modalities: model
            .input_modalities
            .into_iter()
            .map(llm_modality_to_ffi)
            .collect(),
        output_modalities: model
            .output_modalities
            .into_iter()
            .map(llm_modality_to_ffi)
            .collect(),
        supports_multimodal: model.supports_multimodal,
        supports_tools: model.supports_tools,
        supports_reasoning: model.supports_reasoning,
        supports_structured_output: model.supports_structured_output,
        supports_prompt_caching: model.supports_prompt_caching,
        metadata_sources: model
            .metadata_sources
            .into_iter()
            .map(llm_metadata_source_to_ffi)
            .collect(),
    }
}

fn llm_execution_metadata_to_ffi(metadata: LlmExecutionMetadata) -> FfiLlmExecutionMetadata {
    FfiLlmExecutionMetadata {
        requested_format: llm_response_format_kind_to_ffi(metadata.requested_format),
        applied_format: llm_response_format_kind_to_ffi(metadata.applied_format),
        warnings: metadata.warnings,
        attempts: metadata.attempts,
    }
}

fn llm_token_usage_to_ffi(usage: TokenUsage) -> FfiLlmTokenUsage {
    FfiLlmTokenUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        reasoning_tokens: usage.reasoning_tokens,
    }
}

fn llm_response_format_kind_to_ffi(kind: LlmResponseFormatKind) -> FfiLlmResponseFormatKind {
    match kind {
        LlmResponseFormatKind::Text => FfiLlmResponseFormatKind::Text,
        LlmResponseFormatKind::JsonObject => FfiLlmResponseFormatKind::JsonObject,
        LlmResponseFormatKind::JsonSchema => FfiLlmResponseFormatKind::JsonSchema,
    }
}

fn llm_modality_to_ffi(modality: LlmModality) -> FfiLlmModality {
    match modality {
        LlmModality::Text => FfiLlmModality::Text,
        LlmModality::Image => FfiLlmModality::Image,
        LlmModality::Audio => FfiLlmModality::Audio,
        LlmModality::Video => FfiLlmModality::Video,
        LlmModality::Pdf => FfiLlmModality::Pdf,
    }
}

fn llm_metadata_source_to_ffi(source: LlmModelMetadataSource) -> FfiLlmModelMetadataSource {
    match source {
        LlmModelMetadataSource::Provider => FfiLlmModelMetadataSource::Provider,
        LlmModelMetadataSource::ModelsDev => FfiLlmModelMetadataSource::ModelsDev,
    }
}
