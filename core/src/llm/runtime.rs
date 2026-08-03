use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm::provider_protocol::StandardLlmResponse;
use crate::llm::requests::{LlmConfig, LlmGenerateRequest};
use crate::llm::usage::{LlmGenerateSource, TokenUsage};
use crate::ports::llm::{LlmPortError, LlmPortErrorKind};

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

    pub fn normalize_legacy_options(&mut self) {
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

pub fn validate_completion_request(
    request: &mut LlmCompletionRequest,
) -> Result<(), LlmRuntimeError> {
    crate::llm::requests::validate_llm_config(&request.config).map_err(|error| {
        LlmRuntimeError::InvalidRequest {
            reason: error.reason,
        }
    })?;
    if request.input.trim().is_empty() {
        return Err(LlmRuntimeError::InvalidRequest {
            reason: "Input cannot be empty".to_string(),
        });
    }
    request.normalize_legacy_options();
    validate_completion_options(&request.options)?;
    validate_response_format(&request.options.response_format)
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

pub fn finish_response(
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
