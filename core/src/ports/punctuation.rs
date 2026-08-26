use crate::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::Path;
use std::sync::Arc;

/// Implementation family of a punctuation engine provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PunctuationEngineKind {
    #[default]
    CtTransformerOnnx,
}

/// A loaded punctuation model instance, ready for repeated calls.
pub trait PunctuationModel: Send + Sync {
    /// Adds punctuation and capitalization to `text`.
    ///
    /// Implementations should fail typed instead of silently degrading;
    /// callers decide the fallback policy.
    fn punctuate(&self, text: &str) -> Result<String, AsrPortError>;
}

/// Factory that claims model paths and loads [`PunctuationModel`] instances.
///
/// Model loading is expensive relative to inference, so factories are separate
/// from instances: batch jobs load per run while pooling hosts can cache the
/// returned instances for reuse.
pub trait PunctuationEnginePort: Send + Sync {
    fn engine_kind(&self) -> PunctuationEngineKind;

    /// Whether this engine can process the model at `model_path`.
    fn can_handle(&self, model_path: &Path) -> bool;

    /// Loads an instance from `model_path`.
    fn load(
        &self,
        model_path: &Path,
        num_threads: i32,
    ) -> Result<Arc<dyn PunctuationModel>, AsrPortError>;
}

/// Composition-time set of punctuation engines, mirroring [`crate::ports::vad::VadEngineSet`].
#[derive(Clone, Default)]
pub struct PunctuationEngineSet {
    engines: Vec<Arc<dyn PunctuationEnginePort>>,
}

impl std::fmt::Debug for PunctuationEngineSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PunctuationEngineSet")
            .field("engines", &self.engines.len())
            .finish()
    }
}

impl PunctuationEngineSet {
    pub fn empty() -> Self {
        Self {
            engines: Vec::new(),
        }
    }

    /// Builder-style registration for composition roots.
    pub fn register(mut self, engine: Arc<dyn PunctuationEnginePort>) -> Self {
        self.engines.push(engine);
        self
    }

    pub fn engines(&self) -> &[Arc<dyn PunctuationEnginePort>] {
        &self.engines
    }

    /// Resolves the first engine that can handle a usable `model_path`.
    ///
    /// Returns `None` when the path is absent, empty, or nonexistent so
    /// callers treat it as "no punctuation configured".
    pub fn resolve(&self, model_path: Option<&Path>) -> Option<Arc<dyn PunctuationEnginePort>> {
        let path = model_path?;
        if path.as_os_str().is_empty() || !path.exists() {
            return None;
        }
        self.engines
            .iter()
            .find(|engine| engine.can_handle(path))
            .cloned()
    }
}

/// Applies an optional punctuation model with the shared lenient policy:
/// no model, empty input, or an engine failure all return the original text.
pub fn apply_optional_punctuation(model: Option<&dyn PunctuationModel>, text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    let Some(model) = model else {
        return text.to_string();
    };
    model.punctuate(text).unwrap_or_else(|_| text.to_string())
}

/// Loads the configured punctuation model for one transcription job.
///
/// Absent or empty paths yield `None`. A configured path that does not exist
/// is a typed [`AsrPortErrorKind::Model`] error so misconfiguration surfaces
/// loudly; an existing path that no engine claims yields `None` (unwired
/// hosts degrade gracefully).
pub fn load_configured_punctuation(
    engines: &PunctuationEngineSet,
    model_path: Option<&Path>,
) -> Result<Option<Arc<dyn PunctuationModel>>, AsrPortError> {
    let Some(path) = model_path else {
        return Ok(None);
    };
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    if !path.exists() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Model path does not exist: {}", path.display()),
        ));
    }
    match engines.resolve(Some(path)) {
        Some(engine) => Ok(Some(engine.load(path, 1)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PunctuationEngineKind, PunctuationEnginePort, PunctuationEngineSet, PunctuationModel,
        apply_optional_punctuation, load_configured_punctuation,
    };
    use crate::ports::asr::{AsrPortError, AsrPortErrorKind};
    use std::path::Path;
    use std::sync::Arc;

    struct FakeEngine;

    impl PunctuationEnginePort for FakeEngine {
        fn engine_kind(&self) -> PunctuationEngineKind {
            PunctuationEngineKind::CtTransformerOnnx
        }

        fn can_handle(&self, _model_path: &Path) -> bool {
            true
        }

        fn load(
            &self,
            _model_path: &Path,
            _num_threads: i32,
        ) -> Result<Arc<dyn PunctuationModel>, AsrPortError> {
            Ok(Arc::new(FakeModel))
        }
    }

    struct FakeModel;

    impl PunctuationModel for FakeModel {
        fn punctuate(&self, text: &str) -> Result<String, AsrPortError> {
            Ok(format!("{text}。"))
        }
    }

    struct FailingModel;

    impl PunctuationModel for FailingModel {
        fn punctuate(&self, _text: &str) -> Result<String, AsrPortError> {
            Err(AsrPortError::runtime("boom"))
        }
    }

    #[test]
    fn empty_set_resolves_nothing() {
        assert!(
            PunctuationEngineSet::empty()
                .resolve(Some(Path::new("missing")))
                .is_none()
        );
    }

    #[test]
    fn resolve_rejects_missing_and_empty_paths() {
        let set = PunctuationEngineSet::empty().register(Arc::new(FakeEngine));

        assert!(set.resolve(None).is_none());
        assert!(set.resolve(Some(Path::new(""))).is_none());
        assert!(
            set.resolve(Some(Path::new("definitely/not/a/model.onnx")))
                .is_none()
        );
    }

    #[test]
    fn resolve_returns_the_first_engine_and_loads_instances() {
        let set = PunctuationEngineSet::empty().register(Arc::new(FakeEngine));
        let existing = std::env::temp_dir();

        let engine = set.resolve(Some(&existing)).expect("engine resolves");
        assert_eq!(
            engine.engine_kind(),
            PunctuationEngineKind::CtTransformerOnnx
        );

        let model = engine.load(&existing, 1).expect("loads");
        assert_eq!(model.punctuate("你好").unwrap(), "你好。");
    }

    #[test]
    fn apply_optional_punctuation_falls_back_to_original_text() {
        let model = FakeModel;

        assert_eq!(
            apply_optional_punctuation(Some(&model), "你好世界"),
            "你好世界。"
        );
        assert_eq!(apply_optional_punctuation(None, "你好世界"), "你好世界");
        assert_eq!(apply_optional_punctuation(Some(&model), "   "), "   ");
        assert_eq!(
            apply_optional_punctuation(Some(&FailingModel), "你好"),
            "你好"
        );
    }

    #[test]
    fn load_errors_carry_the_port_error_contract() {
        let error = AsrPortError::new(AsrPortErrorKind::Model, "missing");

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert_eq!(error.code(), "MODEL_ERROR");
    }

    #[test]
    fn configured_load_treats_missing_paths_as_model_errors() {
        let set = PunctuationEngineSet::empty().register(Arc::new(FakeEngine));
        let missing = Path::new("definitely/not/a/model.onnx");

        let error = match load_configured_punctuation(&set, Some(missing)) {
            Ok(_) => panic!("expected a model error"),
            Err(error) => error,
        };

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("Model path does not exist"));
    }

    #[test]
    fn configured_load_yields_none_for_unconfigured_paths() {
        let set = PunctuationEngineSet::empty().register(Arc::new(FakeEngine));

        assert!(load_configured_punctuation(&set, None).unwrap().is_none());
        assert!(
            load_configured_punctuation(&set, Some(Path::new("")))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn configured_load_loads_through_the_first_engine() {
        let set = PunctuationEngineSet::empty().register(Arc::new(FakeEngine));
        let existing = std::env::temp_dir();

        let model = load_configured_punctuation(&set, Some(&existing))
            .unwrap()
            .expect("loads");

        assert_eq!(model.punctuate("你好").unwrap(), "你好。");
    }
}
