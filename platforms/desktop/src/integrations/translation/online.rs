use async_trait::async_trait;
use sona_core::llm::requests::LlmConfig;
use sona_core::ports::llm::{LlmTranslationPort, LlmTranslationRequest};
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};

use crate::integrations::llm::DesktopLlmAdapter;

/// Adapter implementing `TranslationEnginePort` for online translation services
/// (e.g. Google Translate, Google Translate Free, and online LLM providers).
pub struct OnlineStreamingTranslationAdapter {
    desktop_llm: DesktopLlmAdapter,
    config: LlmConfig,
}

impl OnlineStreamingTranslationAdapter {
    pub(crate) fn new(desktop_llm: DesktopLlmAdapter, config: LlmConfig) -> Self {
        Self {
            desktop_llm,
            config,
        }
    }
}

#[async_trait]
impl TranslationEnginePort for OnlineStreamingTranslationAdapter {
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

        let req = LlmTranslationRequest {
            config: self.config.clone(),
            texts: vec![trimmed.to_string()],
            source_language: Some(source_lang.to_string()),
            target_language: target_lang.to_string(),
        };

        let mut results = self
            .desktop_llm
            .translate_batch(req)
            .await
            .map_err(|e| TranslationPortError::engine_failed(e.to_string()))?;

        results.pop().ok_or_else(|| {
            TranslationPortError::engine_failed(
                "Empty translation result returned from online translation provider",
            )
        })
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::Online
    }
}
