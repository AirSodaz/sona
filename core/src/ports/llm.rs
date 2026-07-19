use crate::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use std::time::Duration;

use crate::llm::requests::{LlmConfig, LlmGenerateRequest, LlmModelsRequest};
use crate::llm::runtime::{LlmCompletionRequest, LlmStreamDelta};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmPortErrorKind {
    InvalidRequest,
    Authentication,
    Permission,
    RateLimited,
    Timeout,
    Unavailable,
    Unsupported,
    Protocol,
    Network,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct LlmPortError {
    pub kind: LlmPortErrorKind,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}

impl LlmPortError {
    pub fn new(kind: LlmPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retry_after_ms: None,
        }
    }
}

#[async_trait]
pub trait LlmCompletionPort: Send + Sync {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError>;
}

#[async_trait]
pub trait LlmModelMetadataPort: Send + Sync {
    async fn describe_model(
        &self,
        config: &crate::llm::requests::LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError>;
}

#[async_trait]
pub trait LlmModelDiscoveryPort: Send + Sync {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError>;
}

#[async_trait]
pub trait LlmStreamingPort: Send + Sync {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError>;
}

#[derive(Clone, Debug)]
pub struct LlmTranslationRequest {
    pub config: LlmConfig,
    pub texts: Vec<String>,
    pub target_language: String,
}

#[async_trait]
pub trait LlmTranslationPort: Send + Sync {
    async fn translate_batch(
        &self,
        request: LlmTranslationRequest,
    ) -> Result<Vec<String>, LlmPortError>;
}

#[async_trait]
pub trait LlmTaskDelayPort: Send + Sync {
    async fn delay(&self, duration: Duration);
}

pub trait LlmTaskRuntimePort:
    Clone
    + LlmCompletionPort
    + LlmModelMetadataPort
    + LlmStreamingPort
    + LlmTranslationPort
    + LlmTaskDelayPort
    + Send
    + Sync
{
}

impl<T> LlmTaskRuntimePort for T where
    T: Clone
        + LlmCompletionPort
        + LlmModelMetadataPort
        + LlmStreamingPort
        + LlmTranslationPort
        + LlmTaskDelayPort
        + Send
        + Sync
{
}

#[async_trait]
pub trait LlmTextGenerator: Send + Sync {
    async fn generate_text(
        &self,
        request: LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, LlmPortError>;
}

#[async_trait]
pub trait LlmModelLister: Send + Sync {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError>;
}
