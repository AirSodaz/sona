use crate::audio::resolve_model_onnx_path;
use sherpa_onnx::{OfflinePunctuation, OfflinePunctuationConfig, OfflinePunctuationModelConfig};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::Path;

pub struct Punctuation {
    inner: OfflinePunctuation,
}

impl Punctuation {
    pub fn new(model_path: &str, num_threads: i32) -> Result<Self, AsrPortError> {
        let config = OfflinePunctuationConfig {
            model: OfflinePunctuationModelConfig {
                ct_transformer: Some(model_path.to_string()),
                num_threads,
                debug: false,
                provider: Some("cpu".to_string()),
            },
        };

        let inner = OfflinePunctuation::create(&config).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "Failed to create OfflinePunctuation",
            )
        })?;

        Ok(Self { inner })
    }

    pub fn add_punct(&self, text: &str) -> String {
        self.inner
            .add_punctuation(text)
            .unwrap_or_else(|| text.to_string())
    }
}

unsafe impl Send for Punctuation {}
unsafe impl Sync for Punctuation {}

pub fn load_punctuation(punctuation_model: Option<String>) -> Option<Punctuation> {
    let p_path = punctuation_model?;
    if p_path.is_empty() {
        return None;
    }

    load_punctuation_from_path(Some(Path::new(&p_path)))
        .ok()
        .flatten()
}

pub fn load_punctuation_from_path(
    punctuation_model: Option<&Path>,
) -> Result<Option<Punctuation>, AsrPortError> {
    let Some(path) = punctuation_model else {
        return Ok(None);
    };

    if path.as_os_str().is_empty() {
        return Ok(None);
    }

    let model_path = resolve_model_onnx_path(path)?;
    Punctuation::new(&model_path.to_string_lossy(), 1).map(Some)
}
