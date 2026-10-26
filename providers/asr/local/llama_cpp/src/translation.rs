//! LlamaCpp translation adapter implementing TranslationEnginePort.

use crate::llm::LlamaCppLlmEngine;
use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{LlmCompletionOptions, LlmCompletionRequest};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortErrorKind;
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
        let src = source_lang.trim();
        let tgt = target_lang.trim();
        if src.is_empty() || tgt.is_empty() {
            return Err(TranslationPortError::invalid_language(
                "Source and target languages cannot be empty",
            ));
        }

        let trimmed = text.trim();
        if trimmed.is_empty() || src.eq_ignore_ascii_case(tgt) {
            return Ok(trimmed.to_string());
        }
        let system_prompt = format!(
            "You are a professional real-time translator. Translate the text inside <text_to_translate> from {src} to {tgt}. Output only the translation without any explanation, markdown formatting, or commentary."
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
                timeout_seconds: Some(30),
            },
            system_prompt: Some(system_prompt),
            input: format!("<text_to_translate>\n{trimmed}\n</text_to_translate>"),
            options: LlmCompletionOptions {
                max_output_tokens: Some((trimmed.len() as u64).saturating_mul(2).clamp(256, 2048)),
                ..Default::default()
            },
            source: None,
        };

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.engine.execute_completion_internal(request, Some(tx)),
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
                    "Translation timed out after 30 seconds",
                ));
            }
        };
        Ok(strip_translation_tags(&response.text))
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::LlamaCpp
    }
}

fn strip_translation_tags(text: &str) -> String {
    let mut result = text.trim().to_string();
    if let Some(idx) = result.find("<translation>") {
        result = result[idx + "<translation>".len()..].to_string();
    }
    if let Some(idx) = result.rfind("</translation>") {
        result = result[..idx].to_string();
    }
    result.trim().to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::translation::TranslationPortErrorKind;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_adapter_properties_and_passthrough() {
        let engine = Arc::new(LlamaCppLlmEngine::new());
        let adapter = LlamaCppTranslationAdapter::new(engine, "test-model");

        // engine_kind
        assert_eq!(adapter.engine_kind(), TranslationEngineKind::LlamaCpp);

        // empty language error
        let err1 = adapter.translate("hello", "", "zh").await.unwrap_err();
        assert_eq!(err1.kind, TranslationPortErrorKind::InvalidLanguage);

        let err2 = adapter.translate("hello", "en", "   ").await.unwrap_err();
        assert_eq!(err2.kind, TranslationPortErrorKind::InvalidLanguage);

        // whitespace passthrough
        let res = adapter.translate("   ", "en", "zh").await.unwrap();
        assert_eq!(res, "");

        // identical language passthrough
        let res2 = adapter.translate("Hello world", "en", "EN").await.unwrap();
        assert_eq!(res2, "Hello world");

        let res3 = adapter
            .translate("Hello world", "  en  ", "en")
            .await
            .unwrap();
        assert_eq!(res3, "Hello world");
    }

    #[test]
    fn test_strip_translation_tags() {
        assert_eq!(
            strip_translation_tags("<translation>Hello world</translation>"),
            "Hello world"
        );
        assert_eq!(
            strip_translation_tags("  <translation>\n你好世界\n</translation>  "),
            "你好世界"
        );
        assert_eq!(
            strip_translation_tags("Plain text without tags"),
            "Plain text without tags"
        );
    }
}
