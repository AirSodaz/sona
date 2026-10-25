//! LlamaCpp translation adapter implementing TranslationEnginePort.

use crate::llm::LlamaCppLlmEngine;
use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{LlmCompletionOptions, LlmCompletionRequest};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmCompletionPort, LlmPortErrorKind};
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};
use std::sync::Arc;

pub struct LlamaCppTranslationAdapter {
    engine: Arc<LlamaCppLlmEngine>,
    model_name: String,
}

impl LlamaCppTranslationAdapter {
    pub fn new(engine: Arc<LlamaCppLlmEngine>, model_name: impl Into<String>) -> Self {
        Self {
            engine,
            model_name: model_name.into(),
        }
    }
}

#[async_trait]
impl TranslationEnginePort for LlamaCppTranslationAdapter {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        let trimmed = text.trim();
        if trimmed.is_empty() || source_lang.eq_ignore_ascii_case(target_lang) {
            return Ok(trimmed.to_string());
        }
        let system_prompt = format!(
            "You are a professional real-time translator. Translate the following text from {source_lang} to {target_lang}. Output only the translation without any explanation, markdown formatting, or commentary."
        );

        let request = LlmCompletionRequest {
            config: LlmConfig {
                provider: LlmProvider::Builtin(BuiltinLlmProvider::Local),
                strategy: LlmProviderStrategy::Local,
                base_url: String::new(),
                api_key: String::new(),
                model: self.model_name.clone(),
                api_path: None,
                api_version: None,
                temperature: Some(0.0),
                reasoning_enabled: Some(false),
                reasoning_level: None,
                timeout_seconds: Some(10),
            },
            system_prompt: Some(system_prompt),
            input: trimmed.to_string(),
            options: LlmCompletionOptions {
                max_output_tokens: Some(256),
                ..Default::default()
            },
            source: None,
        };

        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.engine.complete(request),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(llm_err)) => {
                return Err(match llm_err.kind {
                    LlmPortErrorKind::Unavailable => {
                        TranslationPortError::model_not_ready(llm_err.message)
                    }
                    _ => TranslationPortError::inference_failed(llm_err.message),
                });
            }
            Err(_) => {
                return Err(TranslationPortError::engine_failed(
                    "Translation timed out after 10 seconds",
                ));
            }
        };
        Ok(response.text.trim().to_string())
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::LlamaCpp
    }
}
