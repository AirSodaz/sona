//! LlamaCpp translation adapter implementing TranslationEnginePort.

use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmCompletionPort;
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};
use std::sync::Arc;

use crate::llm::LlamaCppLlmEngine;

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
        if trimmed.is_empty() {
            return Ok(String::new());
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
            options: Default::default(),
            source: None,
        };

        let response = self
            .engine
            .complete(request)
            .await
            .map_err(TranslationPortError::inference_failed)?;

        Ok(response.text.trim().to_string())
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::LlamaCpp
    }
}
