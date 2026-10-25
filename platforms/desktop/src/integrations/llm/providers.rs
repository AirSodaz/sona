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

#[derive(Clone, Default)]
pub(crate) struct DesktopLlmAdapter {
    online: OnlineLlmAdapter,
    local: LlamaCppLlmEngine,
    models_dir: Option<PathBuf>,
    translation_pool:
        Option<std::sync::Arc<crate::integrations::translation::TranslationModelPool>>,
}

fn detect_source_language(texts: &[String]) -> &'static str {
    for text in texts {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        for ch in trimmed.chars() {
            if ('\u{3040}'..='\u{309F}').contains(&ch) || ('\u{30A0}'..='\u{30FF}').contains(&ch) {
                return "ja";
            }
        }
        for ch in trimmed.chars() {
            if ('\u{4E00}'..='\u{9FFF}').contains(&ch) {
                return "zh";
            }
        }
        return "en";
    }
    "en"
}

impl DesktopLlmAdapter {
    pub(crate) fn new(models_dir: Option<PathBuf>) -> Self {
        Self::with_pool(models_dir, None)
    }

    pub(crate) fn with_pool(
        models_dir: Option<PathBuf>,
        translation_pool: Option<
            std::sync::Arc<crate::integrations::translation::TranslationModelPool>,
        >,
    ) -> Self {
        Self {
            online: OnlineLlmAdapter,
            local: LlamaCppLlmEngine::with_models_dir(models_dir.clone()),
            models_dir,
            translation_pool,
        }
    }

    pub(crate) fn is_local_config(config: &LlmConfig) -> bool {
        matches!(
            config.provider,
            LlmProvider::Builtin(BuiltinLlmProvider::Local)
        ) || config.strategy == LlmProviderStrategy::Local
            || config.strategy == LlmProviderStrategy::LocalNmt
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
        let is_nmt = request.config.strategy == LlmProviderStrategy::LocalNmt
            || (Self::is_local_config(&request.config)
                && (request.config.model.to_lowercase().contains("opus")
                    || request.config.model.to_lowercase() == "auto"
                    || request.config.model.is_empty()));

        if is_nmt {
            let Some(models_dir) = &self.models_dir else {
                return Err(LlmPortError::new(
                    LlmPortErrorKind::Unavailable,
                    "Models directory not configured for local NMT",
                ));
            };
            let pool = self.translation_pool.clone().unwrap_or_else(|| {
                std::sync::Arc::new(crate::integrations::translation::TranslationModelPool::new())
            });

            let src_lang = request
                .source_language
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| detect_source_language(&request.texts));
            let tgt_lang = &request.target_language;

            let adapter = pool
                .get_or_load(src_lang, tgt_lang, models_dir)
                .map_err(|e| {
                    LlmPortError::new(
                        LlmPortErrorKind::Unavailable,
                        format!("Failed to load local NMT model for {src_lang}->{tgt_lang}: {e}"),
                    )
                })?;

            use sona_core::ports::translation::TranslationEnginePort;
            let mut results = Vec::with_capacity(request.texts.len());
            for text in &request.texts {
                let translated =
                    adapter
                        .translate(text, src_lang, tgt_lang)
                        .await
                        .map_err(|e| {
                            LlmPortError::new(
                                LlmPortErrorKind::Unavailable,
                                format!("Local NMT translation failed: {e}"),
                            )
                        })?;
                results.push(translated);
            }
            Ok(results)
        } else if Self::is_local_config(&request.config) {
            Err(LlmPortError::new(
                LlmPortErrorKind::Unsupported,
                "Local LLM models use prompt-based translation",
            ))
        } else {
            self.online.translate_batch(request).await
        }
    }
}

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
