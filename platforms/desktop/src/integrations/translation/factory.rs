use std::path::Path;
use std::sync::Arc;

use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};

use crate::services::DesktopServices;

pub struct TranslationEngineFactory;

impl TranslationEngineFactory {
    pub fn create_engine(
        services: &DesktopServices,
        engine_kind: TranslationEngineKind,
        model_name_or_id: Option<&str>,
        source_lang: &str,
        target_lang: &str,
        models_dir: &Path,
    ) -> Result<Arc<dyn TranslationEnginePort>, TranslationPortError> {
        match engine_kind {
            TranslationEngineKind::OnnxNmt => {
                let adapter =
                    services
                        .translation_pool
                        .get_or_load(source_lang, target_lang, models_dir)?;
                Ok(adapter as Arc<dyn TranslationEnginePort>)
            }
            TranslationEngineKind::LlamaCpp => {
                let model_name = match model_name_or_id {
                    Some(m) if !m.trim().is_empty() => m.trim(),
                    _ => "",
                };
                let engine = Arc::new(sona_llama_cpp::LlamaCppLlmEngine::with_models_dir(Some(
                    models_dir.to_path_buf(),
                )));
                let adapter = Arc::new(
                    sona_llama_cpp::translation::LlamaCppTranslationAdapter::new(
                        engine, model_name,
                    ),
                );
                Ok(adapter as Arc<dyn TranslationEnginePort>)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::database::DesktopSqliteState;
    use crate::platform::event::MockEventEmitter;
    use crate::services::DesktopServices;

    fn create_test_services(temp_dir: &std::path::Path) -> DesktopServices {
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp_dir).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let emitter = Arc::new(MockEventEmitter::new());
        DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .build()
            .unwrap()
    }

    #[test]
    fn factory_creates_llamacpp_engine_successfully() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = create_test_services(temp_dir.path());
        let engine = TranslationEngineFactory::create_engine(
            &services,
            TranslationEngineKind::LlamaCpp,
            Some("test-gguf-model"),
            "en",
            "zh",
            temp_dir.path(),
        )
        .unwrap();

        assert_eq!(engine.engine_kind(), TranslationEngineKind::LlamaCpp);
    }

    #[test]
    fn factory_creates_onnx_engine_or_fails_when_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = create_test_services(temp_dir.path());
        let result = TranslationEngineFactory::create_engine(
            &services,
            TranslationEngineKind::OnnxNmt,
            None,
            "en",
            "zh",
            temp_dir.path(),
        );

        let err = result
            .err()
            .expect("expected ONNX model load to fail with empty dir");
        assert_eq!(
            err.kind,
            sona_core::ports::translation::TranslationPortErrorKind::ModelNotReady
        );
    }
}
