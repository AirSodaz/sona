use crate::audio::resolve_model_onnx_path;
use sherpa_onnx::{
    OfflinePunctuation, OfflinePunctuationConfig, OfflinePunctuationModelConfig, OnlinePunctuation,
    OnlinePunctuationConfig, OnlinePunctuationModelConfig,
};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::{Path, PathBuf};

enum PunctuationInner {
    Offline(OfflinePunctuation),
    Online(OnlinePunctuation),
    #[cfg(test)]
    TestDummy,
}

pub struct Punctuation {
    inner: PunctuationInner,
}

fn resolve_vocab_path(model_path: &Path) -> Option<PathBuf> {
    let dir = if model_path.is_dir() {
        model_path
    } else {
        model_path.parent()?
    };
    let candidate = dir.join("bpe.vocab");
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

impl Punctuation {
    #[cfg(test)]
    pub fn test_dummy() -> Self {
        Self {
            inner: PunctuationInner::TestDummy,
        }
    }

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

        Ok(Self {
            inner: PunctuationInner::Offline(inner),
        })
    }

    pub fn new_online(
        model_path: &str,
        vocab_path: &str,
        num_threads: i32,
    ) -> Result<Self, AsrPortError> {
        let config = OnlinePunctuationConfig {
            model: OnlinePunctuationModelConfig {
                cnn_bilstm: Some(model_path.to_string()),
                bpe_vocab: Some(vocab_path.to_string()),
                num_threads,
                debug: false,
                provider: Some("cpu".to_string()),
            },
        };

        let inner = OnlinePunctuation::create(&config).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "Failed to create OnlinePunctuation",
            )
        })?;

        Ok(Self {
            inner: PunctuationInner::Online(inner),
        })
    }

    pub fn add_punct(&self, text: &str) -> String {
        match &self.inner {
            PunctuationInner::Offline(punct) => punct
                .add_punctuation(text)
                .unwrap_or_else(|| text.to_string()),
            PunctuationInner::Online(punct) => punct
                .add_punctuation(text)
                .unwrap_or_else(|| text.to_string()),
            #[cfg(test)]
            PunctuationInner::TestDummy => text.to_string(),
        }
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
    if let Some(vocab_path) = resolve_vocab_path(path) {
        Punctuation::new_online(
            &model_path.to_string_lossy(),
            &vocab_path.to_string_lossy(),
            1,
        )
        .map(Some)
    } else {
        Punctuation::new(&model_path.to_string_lossy(), 1).map(Some)
    }
}
