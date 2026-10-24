use sherpa_onnx::{OnlinePunctuation, OnlinePunctuationConfig, OnlinePunctuationModelConfig};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::ports::punctuation::{
    PunctuationEngineKind, PunctuationEnginePort, PunctuationModel,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::shared::{NUM_THREADS, resolve_model_onnx_path};

/// CNN-BiLSTM ONNX online punctuation engine backed by sherpa-onnx.
///
/// Claims models that include both an ONNX model file and a `bpe.vocab` vocabulary.
#[derive(Debug, Clone, Copy, Default)]
pub struct CnnBiLstmEngine;

pub(crate) fn resolve_vocab_path(model_path: &Path) -> Option<PathBuf> {
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

impl PunctuationEnginePort for CnnBiLstmEngine {
    fn engine_kind(&self) -> PunctuationEngineKind {
        PunctuationEngineKind::CnnBiLstmOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        if !model_path.exists() {
            return false;
        }
        let has_vocab = resolve_vocab_path(model_path).is_some();
        let has_onnx = resolve_model_onnx_path(model_path).is_ok();
        has_vocab && has_onnx
    }

    fn load(
        &self,
        model_path: &Path,
        num_threads: i32,
    ) -> Result<Arc<dyn PunctuationModel>, AsrPortError> {
        let onnx_path = resolve_model_onnx_path(model_path)?;
        let vocab_path = resolve_vocab_path(model_path).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!(
                    "bpe.vocab not found for online punctuation model in {}",
                    model_path.display()
                ),
            )
        })?;
        let threads = if num_threads > 0 {
            num_threads
        } else {
            NUM_THREADS
        };
        Ok(Arc::new(CnnBiLstmModel::new(
            &onnx_path.to_string_lossy(),
            &vocab_path.to_string_lossy(),
            threads,
        )?))
    }
}

/// Loaded CNN-BiLSTM online punctuation instance wrapping sherpa-onnx.
pub struct CnnBiLstmModel {
    inner: OnlinePunctuation,
}

impl CnnBiLstmModel {
    pub fn new(model_path: &str, vocab_path: &str, num_threads: i32) -> Result<Self, AsrPortError> {
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

        Ok(Self { inner })
    }
}

impl PunctuationModel for CnnBiLstmModel {
    fn punctuate(&self, text: &str) -> Result<String, AsrPortError> {
        Ok(self
            .inner
            .add_punctuation(text)
            .unwrap_or_else(|| text.to_string()))
    }
}

unsafe impl Send for CnnBiLstmModel {}
unsafe impl Sync for CnnBiLstmModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn engine_kind_is_cnn_bilstm() {
        let engine = CnnBiLstmEngine;
        assert_eq!(engine.engine_kind(), PunctuationEngineKind::CnnBiLstmOnnx);
    }

    #[test]
    fn can_handle_requires_both_vocab_and_onnx() {
        let engine = CnnBiLstmEngine;
        let root =
            std::env::temp_dir().join(format!("sona-cnn-bilstm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        assert!(!engine.can_handle(&root));

        let onnx_file = root.join("model.onnx");
        fs::write(&onnx_file, b"dummy onnx").unwrap();
        assert!(!engine.can_handle(&root));
        assert!(!engine.can_handle(&onnx_file));

        let vocab_file = root.join("bpe.vocab");
        fs::write(&vocab_file, b"dummy vocab").unwrap();
        assert!(engine.can_handle(&root));
        assert!(engine.can_handle(&onnx_file));

        let missing = root.join("does-not-exist");
        assert!(!engine.can_handle(&missing));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_reports_missing_models_as_model_errors() {
        let missing = std::env::temp_dir().join(format!("sona-cnn-missing-{}", std::process::id()));
        let error = match CnnBiLstmEngine.load(&missing, 1) {
            Ok(_) => panic!("expected a model error"),
            Err(e) => e,
        };
        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("Model path does not exist"));
    }

    #[test]
    fn load_rejects_directory_without_vocab() {
        let root = std::env::temp_dir().join(format!("sona-cnn-no-vocab-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("model.onnx"), b"fake onnx").unwrap();

        let error = match CnnBiLstmEngine.load(&root, 1) {
            Ok(_) => panic!("expected a model error"),
            Err(e) => e,
        };
        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("bpe.vocab not found"));

        let _ = fs::remove_dir_all(&root);
    }
}
