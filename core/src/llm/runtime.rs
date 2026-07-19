use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm::provider_protocol::LlmModelSummary;
use crate::llm::provider_protocol::StandardLlmResponse;
use crate::llm::requests::{LlmConfig, LlmGenerateRequest, LlmModelsRequest, validate_llm_config};
use crate::llm::usage::{LlmGenerateSource, TokenUsage};
use crate::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortError, LlmPortErrorKind,
    LlmStreamingPort,
};

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LlmResponseFormat {
    #[default]
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
        schema: Value,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmPromptCachePolicy {
    #[default]
    Disabled,
    Automatic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmCapabilityPolicy {
    Strict,
    #[default]
    Compatible,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase", default)]
pub struct LlmCompletionOptions {
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub temperature: Option<f32>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub max_output_tokens: Option<u64>,
    pub reasoning_enabled: Option<bool>,
    pub reasoning_level: Option<String>,
    pub response_format: LlmResponseFormat,
    pub prompt_cache: LlmPromptCachePolicy,
    pub capability_policy: LlmCapabilityPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmCompletionRequest {
    pub config: LlmConfig,
    #[serde(default)]
    pub system_prompt: Option<String>,
    pub input: String,
    #[serde(default)]
    pub options: LlmCompletionOptions,
    #[serde(default)]
    pub source: Option<LlmGenerateSource>,
}

impl LlmCompletionRequest {
    pub fn effective_temperature(&self) -> Option<f32> {
        self.options.temperature.or(self.config.temperature)
    }

    pub fn effective_reasoning_enabled(&self) -> bool {
        self.options
            .reasoning_enabled
            .or(self.config.reasoning_enabled)
            .unwrap_or(false)
    }

    pub fn effective_reasoning_level(&self) -> Option<&str> {
        self.options
            .reasoning_level
            .as_deref()
            .or(self.config.reasoning_level.as_deref())
    }

    fn normalize_legacy_options(&mut self) {
        if self.options.temperature.is_none() {
            self.options.temperature = self.config.temperature;
        }
        if self.options.reasoning_enabled.is_none() {
            self.options.reasoning_enabled = self.config.reasoning_enabled;
        }
        if self.options.reasoning_level.is_none() {
            self.options.reasoning_level = self.config.reasoning_level.clone();
        }
    }
}

impl From<LlmGenerateRequest> for LlmCompletionRequest {
    fn from(request: LlmGenerateRequest) -> Self {
        Self {
            options: LlmCompletionOptions {
                temperature: request.config.temperature,
                reasoning_enabled: request.config.reasoning_enabled,
                reasoning_level: request.config.reasoning_level.clone(),
                ..LlmCompletionOptions::default()
            },
            config: request.config,
            system_prompt: None,
            input: request.input,
            source: request.source,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmResponseFormatKind {
    Text,
    JsonObject,
    JsonSchema,
}

impl From<&LlmResponseFormat> for LlmResponseFormatKind {
    fn from(value: &LlmResponseFormat) -> Self {
        match value {
            LlmResponseFormat::Text => Self::Text,
            LlmResponseFormat::JsonObject => Self::JsonObject,
            LlmResponseFormat::JsonSchema { .. } => Self::JsonSchema,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmExecutionMetadata {
    pub requested_format: LlmResponseFormatKind,
    pub applied_format: LlmResponseFormatKind,
    pub warnings: Vec<String>,
    pub attempts: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmCompletionResponse {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Unknown>)
    )]
    pub json: Option<Value>,
    pub usage: Option<TokenUsage>,
    pub execution: LlmExecutionMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmStreamDelta {
    pub text: String,
    pub delta: String,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LlmRuntimeError {
    #[error("{reason}")]
    InvalidRequest { reason: String },
    #[error("Model '{model}' does not support {capability}")]
    UnsupportedCapability { model: String, capability: String },
    #[error("{reason}")]
    InvalidResponse { reason: String },
    #[error("{reason}")]
    Adapter {
        kind: LlmPortErrorKind,
        reason: String,
        retry_after_ms: Option<u64>,
    },
}

impl From<LlmPortError> for LlmRuntimeError {
    fn from(error: LlmPortError) -> Self {
        Self::Adapter {
            kind: error.kind,
            reason: error.message,
            retry_after_ms: error.retry_after_ms,
        }
    }
}

pub struct LlmRuntimeService<'a, Completion, Metadata> {
    completion: &'a Completion,
    metadata: Metadata,
}

impl<'a, Completion, Metadata> LlmRuntimeService<'a, Completion, Metadata>
where
    Completion: LlmCompletionPort,
    Metadata: LlmModelMetadataPort,
{
    pub fn new(completion: &'a Completion, metadata: Metadata) -> Self {
        Self {
            completion,
            metadata,
        }
    }

    pub async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmRuntimeError> {
        let prepared = self.prepare_request(request).await?;
        let applied_format = prepared.request.options.response_format.clone();
        let response = self.completion.complete(prepared.request).await?;
        finish_response(
            response,
            prepared.requested_format,
            applied_format,
            prepared.validation_format,
            prepared.warnings,
        )
    }

    pub async fn stream(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<LlmCompletionResponse, LlmRuntimeError>
    where
        Completion: LlmStreamingPort,
    {
        let prepared = self.prepare_request(request).await?;
        let applied_format = prepared.request.options.response_format.clone();
        let response = self
            .completion
            .stream_completion(prepared.request, emit_delta)
            .await?;
        finish_response(
            response,
            prepared.requested_format,
            applied_format,
            prepared.validation_format,
            prepared.warnings,
        )
    }

    async fn prepare_request(
        &self,
        mut request: LlmCompletionRequest,
    ) -> Result<PreparedRequest, LlmRuntimeError> {
        validate_llm_config(&request.config).map_err(|error| LlmRuntimeError::InvalidRequest {
            reason: error.reason,
        })?;
        if request.input.trim().is_empty() {
            return Err(LlmRuntimeError::InvalidRequest {
                reason: "Input cannot be empty".to_string(),
            });
        }
        request.normalize_legacy_options();
        validate_completion_options(&request.options)?;
        validate_response_format(&request.options.response_format)?;

        let requested_format = LlmResponseFormatKind::from(&request.options.response_format);
        let validation_format = request.options.response_format.clone();
        let mut warnings = Vec::new();
        let requested_schema = match &request.options.response_format {
            LlmResponseFormat::JsonSchema { schema, .. } => Some(schema.clone()),
            _ => None,
        };
        if let Some(schema) = requested_schema
            && self
                .metadata
                .describe_model(&request.config)
                .await?
                .and_then(|model| model.supports_structured_output)
                == Some(false)
        {
            if request.options.capability_policy == LlmCapabilityPolicy::Strict {
                return Err(LlmRuntimeError::UnsupportedCapability {
                    model: request.config.model.clone(),
                    capability: "structured output".to_string(),
                });
            }
            request
                .input
                .push_str("\n\nReturn only a JSON object that satisfies this JSON Schema:\n");
            request.input.push_str(&schema.to_string());
            request.options.response_format = LlmResponseFormat::JsonObject;
            warnings.push(
                "Model metadata reports no structured output support; using JSON object mode"
                    .to_string(),
            );
        }

        Ok(PreparedRequest {
            request,
            requested_format,
            validation_format,
            warnings,
        })
    }

    pub async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmRuntimeError>
    where
        Completion: LlmModelDiscoveryPort,
    {
        self.completion
            .list_models(request)
            .await
            .map_err(Into::into)
    }

    pub async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmRuntimeError> {
        self.metadata
            .describe_model(config)
            .await
            .map_err(Into::into)
    }
}

fn validate_completion_options(options: &LlmCompletionOptions) -> Result<(), LlmRuntimeError> {
    if let Some(temperature) = options.temperature
        && (!temperature.is_finite() || !(0.0..=2.0).contains(&temperature))
    {
        return Err(LlmRuntimeError::InvalidRequest {
            reason: "Temperature must be between 0 and 2".to_string(),
        });
    }
    if options.max_output_tokens == Some(0) {
        return Err(LlmRuntimeError::InvalidRequest {
            reason: "Maximum output tokens must be greater than zero".to_string(),
        });
    }
    Ok(())
}

struct PreparedRequest {
    request: LlmCompletionRequest,
    requested_format: LlmResponseFormatKind,
    validation_format: LlmResponseFormat,
    warnings: Vec<String>,
}

fn finish_response(
    response: StandardLlmResponse,
    requested_format: LlmResponseFormatKind,
    applied_response_format: LlmResponseFormat,
    validation_format: LlmResponseFormat,
    warnings: Vec<String>,
) -> Result<LlmCompletionResponse, LlmRuntimeError> {
    let applied_format = LlmResponseFormatKind::from(&applied_response_format);
    let json = parse_output(&response.text, &validation_format)?;
    Ok(LlmCompletionResponse {
        text: response.text,
        json,
        usage: response.usage,
        execution: LlmExecutionMetadata {
            requested_format,
            applied_format,
            warnings,
            attempts: 1,
        },
    })
}

fn validate_response_format(format: &LlmResponseFormat) -> Result<(), LlmRuntimeError> {
    let LlmResponseFormat::JsonSchema { name, schema } = format else {
        return Ok(());
    };
    if name.trim().is_empty() {
        return Err(LlmRuntimeError::InvalidRequest {
            reason: "JSON Schema name cannot be empty".to_string(),
        });
    }
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|error| LlmRuntimeError::InvalidRequest {
            reason: format!("Invalid JSON Schema: {error}"),
        })
}

fn parse_output(text: &str, format: &LlmResponseFormat) -> Result<Option<Value>, LlmRuntimeError> {
    if matches!(format, LlmResponseFormat::Text) {
        return Ok(None);
    }

    let value =
        serde_json::from_str::<Value>(text).map_err(|error| LlmRuntimeError::InvalidResponse {
            reason: format!("LLM response is not valid JSON: {error}"),
        })?;
    if matches!(format, LlmResponseFormat::JsonObject) && !value.is_object() {
        return Err(LlmRuntimeError::InvalidResponse {
            reason: "LLM response must be a JSON object".to_string(),
        });
    }
    if let LlmResponseFormat::JsonSchema { schema, .. } = format {
        let validator =
            jsonschema::validator_for(schema).map_err(|error| LlmRuntimeError::InvalidRequest {
                reason: format!("Invalid JSON Schema: {error}"),
            })?;
        if let Err(error) = validator.validate(&value) {
            return Err(LlmRuntimeError::InvalidResponse {
                reason: format!("LLM response does not match JSON Schema: {error}"),
            });
        }
    }
    Ok(Some(value))
}
