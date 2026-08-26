use sherpa_onnx::{OfflinePunctuation, OfflinePunctuationConfig, OfflinePunctuationModelConfig};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::ports::punctuation::{
    PunctuationEngineKind, PunctuationEnginePort, PunctuationModel,
};
use std::path::Path;
use std::sync::Arc;

use super::shared::{NUM_THREADS, resolve_model_onnx_path};

/// ct-transformer ONNX punctuation engine backed by sherpa-onnx.
///
/// Acts as the dispatch fallback: any model file that no other engine claims
/// is handled here.
#[derive(Debug, Clone, Copy, Default)]
pub struct CtTransformerEngine;

impl PunctuationEnginePort for CtTransformerEngine {
    fn engine_kind(&self) -> PunctuationEngineKind {
        PunctuationEngineKind::CtTransformerOnnx
    }

    fn can_handle(&self, _model_path: &Path) -> bool {
        true
    }

    fn load(
        &self,
        model_path: &Path,
        num_threads: i32,
    ) -> Result<Arc<dyn PunctuationModel>, AsrPortError> {
        let model_path = resolve_model_onnx_path(model_path)?;
        let threads = if num_threads > 0 {
            num_threads
        } else {
            NUM_THREADS
        };
        Ok(Arc::new(CtTransformerModel::new(
            &model_path.to_string_lossy(),
            threads,
        )?))
    }
}

/// Loaded ct-transformer instance wrapping the sherpa-onnx runtime.
pub struct CtTransformerModel {
    inner: OfflinePunctuation,
}

impl CtTransformerModel {
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
}

impl PunctuationModel for CtTransformerModel {
    fn punctuate(&self, text: &str) -> Result<String, AsrPortError> {
        Ok(self
            .inner
            .add_punctuation(text)
            .unwrap_or_else(|| text.to_string()))
    }
}

unsafe impl Send for CtTransformerModel {}
unsafe impl Sync for CtTransformerModel {}

#[cfg(test)]
mod tests {
    use super::CtTransformerEngine;
    use sona_core::ports::asr::AsrPortErrorKind;
    use sona_core::ports::punctuation::{PunctuationEngineKind, PunctuationEnginePort};
    use std::path::Path;

    #[test]
    fn engine_kind_is_ct_transformer_and_handles_any_path() {
        let engine = CtTransformerEngine;

        assert_eq!(
            engine.engine_kind(),
            PunctuationEngineKind::CtTransformerOnnx
        );
        assert!(engine.can_handle(Path::new("anything")));
    }

    #[test]
    fn load_reports_missing_models_as_model_errors() {
        let missing =
            std::env::temp_dir().join(format!("sona-punct-missing-{}", std::process::id()));

        let error = match CtTransformerEngine.load(&missing, 1) {
            Ok(_) => panic!("expected a model error"),
            Err(error) => error,
        };

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("Model path does not exist"));
    }

    #[test]
    fn load_rejects_directories_without_onnx_files() {
        use std::fs;

        let root =
            std::env::temp_dir().join(format!("sona-punct-empty-dir-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("readme.txt"), b"stub").unwrap();

        let error = match CtTransformerEngine.load(&root, 1) {
            Ok(_) => panic!("expected a model error"),
            Err(error) => error,
        };

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("No .onnx file found"));

        fs::remove_dir_all(root).unwrap();
    }
}
