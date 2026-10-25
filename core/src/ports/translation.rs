use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "kebab-case")]
pub enum TranslationEngineKind {
    OnnxNmt,
    LlamaCpp,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "kebab-case")]
pub enum TranslationPortErrorKind {
    ModelNotReady,
    TokenizationFailed,
    InferenceFailed,
    InvalidLanguage,
    EngineFailed,
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct TranslationPortError {
    pub kind: TranslationPortErrorKind,
    pub message: String,
    pub cause: Option<String>,
}

impl TranslationPortError {
    pub fn new(kind: TranslationPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            cause: None,
        }
    }

    pub fn with_cause(
        kind: TranslationPortErrorKind,
        message: impl Into<String>,
        cause: impl std::fmt::Display,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            cause: Some(cause.to_string()),
        }
    }

    pub fn model_not_ready(message: impl Into<String>) -> Self {
        Self::new(TranslationPortErrorKind::ModelNotReady, message)
    }

    pub fn tokenization_failed(cause: impl std::fmt::Display) -> Self {
        Self::with_cause(
            TranslationPortErrorKind::TokenizationFailed,
            "Failed to tokenize translation input or output",
            cause,
        )
    }

    pub fn inference_failed(cause: impl std::fmt::Display) -> Self {
        Self::with_cause(
            TranslationPortErrorKind::InferenceFailed,
            "Translation inference failed",
            cause,
        )
    }

    pub fn invalid_language(message: impl Into<String>) -> Self {
        Self::new(TranslationPortErrorKind::InvalidLanguage, message)
    }

    pub fn engine_failed(cause: impl std::fmt::Display) -> Self {
        Self::with_cause(
            TranslationPortErrorKind::EngineFailed,
            "Translation engine error",
            cause,
        )
    }
}

#[async_trait]
pub trait TranslationEnginePort: Send + Sync {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError>;

    fn engine_kind(&self) -> TranslationEngineKind;
}
