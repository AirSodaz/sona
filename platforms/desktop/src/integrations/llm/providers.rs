use super::*;
use log::warn;
use tauri::{AppHandle, Emitter};

use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{LlmConfig, LlmModelsRequest};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmStreamDelta};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortError, LlmPortErrorKind,
    LlmStreamingPort, LlmTaskDelayPort, LlmTextGeneratorPort, LlmTranslationPort,
    LlmTranslationRequest,
};
use sona_llama_cpp::LlamaCppLlmEngine;
use sona_online_llm::OnlineLlmAdapter;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub(crate) struct DesktopLlmAdapter {
    online: OnlineLlmAdapter,
    local: LlamaCppLlmEngine,
}

impl DesktopLlmAdapter {
    pub(crate) fn new(models_dir: Option<PathBuf>) -> Self {
        Self {
            online: OnlineLlmAdapter,
            local: LlamaCppLlmEngine::with_models_dir(models_dir),
        }
    }

    pub(crate) fn is_local_config(config: &LlmConfig) -> bool {
        matches!(
            config.provider,
            LlmProvider::Builtin(BuiltinLlmProvider::Local)
        ) || config.strategy == LlmProviderStrategy::Local
    }
}

#[async_trait]
impl LlmTextGeneratorPort for DesktopLlmAdapter {
    async fn generate_text(
        &self,
        request: sona_core::llm::requests::LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        self.complete(request.into()).await
    }
}

#[async_trait]
impl LlmCompletionPort for DesktopLlmAdapter {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        if Self::is_local_config(&request.config) {
            self.local.complete(request).await
        } else {
            self.online.complete(request).await
        }
    }
}

#[async_trait]
impl LlmStreamingPort for DesktopLlmAdapter {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        if Self::is_local_config(&request.config) {
            self.local.stream_completion(request, emit_delta).await
        } else {
            self.online.stream_completion(request, emit_delta).await
        }
    }
}

#[async_trait]
impl LlmModelMetadataPort for DesktopLlmAdapter {
    async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        if Self::is_local_config(config) {
            self.local.describe_model(config).await
        } else {
            self.online.describe_model(config).await
        }
    }
}

#[async_trait]
impl LlmModelDiscoveryPort for DesktopLlmAdapter {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
        let is_local = matches!(
            request.provider,
            LlmProvider::Builtin(BuiltinLlmProvider::Local)
        ) || matches!(request.strategy, Some(LlmProviderStrategy::Local));
        if is_local {
            self.local.list_models(request).await
        } else {
            self.online.list_models(request).await
        }
    }
}

#[async_trait]
impl LlmTaskDelayPort for DesktopLlmAdapter {
    async fn delay(&self, duration: Duration) {
        self.online.delay(duration).await;
    }
}

#[async_trait]
impl LlmTranslationPort for DesktopLlmAdapter {
    async fn translate_batch(
        &self,
        request: LlmTranslationRequest,
    ) -> Result<Vec<String>, LlmPortError> {
        if Self::is_local_config(&request.config) {
            Err(LlmPortError::new(
                LlmPortErrorKind::Unsupported,
                "Local models use prompt-based translation",
            ))
        } else {
            self.online.translate_batch(request).await
        }
    }
}

#[cfg(test)]
pub(crate) use sona_online_llm::build_gemini_generate_content_request_parts_for_reqwest as build_gemini_generate_content_request_parts;

pub(crate) fn emit_llm_usage_event(
    app: &AppHandle,
    config: &LlmConfig,
    category: LlmUsageCategory,
    occurred_at: String,
    usage: Option<TokenUsage>,
) {
    let payload = LlmUsageEventPayload {
        occurred_at,
        provider: config.provider.clone(),
        model: config.model.clone(),
        category,
        usage,
    };

    if let Err(error) = app.emit(LLM_USAGE_RECORDED_EVENT, payload) {
        warn!(
            "[LLM] failed to emit usage event: provider={:?} category={:?} error={}",
            config.provider, category, error
        );
    }
}
