use crate::{FfiLlmConfig, FfiLlmGenerateSourceV1};
use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};
use sona_core::llm::runtime::{
    LlmCapabilityPolicy, LlmCompletionOptions, LlmCompletionRequest, LlmPromptCachePolicy,
    LlmResponseFormat,
};
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

/// `schema_json` is the one dynamic leaf: a JSON Schema document whose shape is
/// defined by the caller's structured-output contract, not by this binding.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmResponseFormatV1 {
    Text,
    JsonObject,
    JsonSchema { name: String, schema_json: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmPromptCachePolicyV1 {
    Disabled,
    Automatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmCapabilityPolicyV1 {
    Strict,
    Compatible,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmCompletionOptionsV1 {
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u64>,
    pub reasoning_enabled: Option<bool>,
    pub reasoning_level: Option<String>,
    pub response_format: FfiLlmResponseFormatV1,
    pub prompt_cache: FfiLlmPromptCachePolicyV1,
    pub capability_policy: FfiLlmCapabilityPolicyV1,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmCompletionRequestV1 {
    pub config: FfiLlmConfig,
    pub system_prompt: Option<String>,
    pub input: String,
    pub options: FfiLlmCompletionOptionsV1,
    pub source: Option<FfiLlmGenerateSourceV1>,
}

impl From<FfiLlmPromptCachePolicyV1> for LlmPromptCachePolicy {
    fn from(value: FfiLlmPromptCachePolicyV1) -> Self {
        match value {
            FfiLlmPromptCachePolicyV1::Disabled => Self::Disabled,
            FfiLlmPromptCachePolicyV1::Automatic => Self::Automatic,
        }
    }
}

impl From<FfiLlmCapabilityPolicyV1> for LlmCapabilityPolicy {
    fn from(value: FfiLlmCapabilityPolicyV1) -> Self {
        match value {
            FfiLlmCapabilityPolicyV1::Strict => Self::Strict,
            FfiLlmCapabilityPolicyV1::Compatible => Self::Compatible,
        }
    }
}

pub(crate) fn llm_completion_request_from_ffi(
    request: FfiLlmCompletionRequestV1,
) -> Result<LlmCompletionRequest, serde_json::Error> {
    let response_format = match request.options.response_format {
        FfiLlmResponseFormatV1::Text => LlmResponseFormat::Text,
        FfiLlmResponseFormatV1::JsonObject => LlmResponseFormat::JsonObject,
        FfiLlmResponseFormatV1::JsonSchema { name, schema_json } => LlmResponseFormat::JsonSchema {
            name,
            schema: serde_json::from_str(&schema_json)?,
        },
    };
    Ok(LlmCompletionRequest {
        config: crate::mapper::llm_config_from_ffi(request.config),
        system_prompt: request.system_prompt,
        input: request.input,
        options: LlmCompletionOptions {
            temperature: request.options.temperature,
            max_output_tokens: request.options.max_output_tokens,
            reasoning_enabled: request.options.reasoning_enabled,
            reasoning_level: request.options.reasoning_level,
            response_format,
            prompt_cache: request.options.prompt_cache.into(),
            capability_policy: request.options.capability_policy.into(),
        },
        source: request.source.map(Into::into),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FfiSecret;
    use crate::mapper::FfiLlmProviderStrategy;
    use serde_json::json;

    fn ffi_config() -> FfiLlmConfig {
        FfiLlmConfig {
            provider_id: "openai".to_string(),
            strategy: FfiLlmProviderStrategy::OpenAi,
            base_url: "https://api.example".to_string(),
            api_key: FfiSecret::new("sk-secret".to_string()),
            model: "gpt-test".to_string(),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
            timeout_seconds: None,
        }
    }

    fn request(response_format: FfiLlmResponseFormatV1) -> FfiLlmCompletionRequestV1 {
        FfiLlmCompletionRequestV1 {
            config: ffi_config(),
            system_prompt: Some("be brief".to_string()),
            input: "hello".to_string(),
            options: FfiLlmCompletionOptionsV1 {
                temperature: Some(0.2),
                max_output_tokens: Some(256),
                reasoning_enabled: Some(false),
                reasoning_level: None,
                response_format,
                prompt_cache: FfiLlmPromptCachePolicyV1::Automatic,
                capability_policy: FfiLlmCapabilityPolicyV1::Strict,
            },
            source: None,
        }
    }

    #[test]
    fn a_completion_request_carries_every_option_across_the_boundary() {
        let core = llm_completion_request_from_ffi(request(FfiLlmResponseFormatV1::JsonObject))
            .expect("core request");

        assert_eq!(core.input, "hello");
        assert_eq!(core.system_prompt.as_deref(), Some("be brief"));
        assert_eq!(core.options.temperature, Some(0.2));
        assert_eq!(core.options.max_output_tokens, Some(256));
        assert_eq!(core.options.reasoning_enabled, Some(false));
        assert_eq!(core.options.response_format, LlmResponseFormat::JsonObject);
        assert_eq!(core.options.prompt_cache, LlmPromptCachePolicy::Automatic);
        assert_eq!(core.options.capability_policy, LlmCapabilityPolicy::Strict);
        assert_eq!(core.config.api_key, "sk-secret");
    }

    #[test]
    fn a_json_schema_leaf_survives_as_a_parsed_document() {
        let schema = json!({"type": "object", "properties": {"ok": {"type": "boolean"}}});
        let core = llm_completion_request_from_ffi(request(FfiLlmResponseFormatV1::JsonSchema {
            name: "result".to_string(),
            schema_json: schema.to_string(),
        }))
        .expect("core request");

        match core.options.response_format {
            LlmResponseFormat::JsonSchema { name, schema: got } => {
                assert_eq!(name, "result");
                assert_eq!(got, schema);
            }
            other => panic!("expected JsonSchema, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_schema_is_rejected_before_any_request_is_issued() {
        let error = llm_completion_request_from_ffi(request(FfiLlmResponseFormatV1::JsonSchema {
            name: "result".to_string(),
            schema_json: "{".to_string(),
        }));

        assert!(error.is_err());
    }
}
