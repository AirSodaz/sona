use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};

use crate::services::DesktopServices;

struct SemaphoreGuardedTranslationEngine {
    inner: Arc<dyn TranslationEnginePort>,
    semaphore: Arc<tokio::sync::Semaphore>,
}

#[async_trait]
impl TranslationEnginePort for SemaphoreGuardedTranslationEngine {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| TranslationPortError::engine_failed("Translation semaphore closed"))?;
        self.inner.translate(text, source_lang, target_lang).await
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        self.inner.engine_kind()
    }
}

pub type ActiveOnnxLease = Option<(
    (String, String),
    crate::integrations::models::ModelLease<sona_translation_onnx::OnnxNmtAdapter>,
)>;

pub struct PooledOnnxTranslationEngine {
    pool: Arc<crate::integrations::models::LocalModelPool>,
    models_dir: PathBuf,
    lease: parking_lot::Mutex<ActiveOnnxLease>,
}

impl PooledOnnxTranslationEngine {
    pub fn new(
        pool: Arc<crate::integrations::models::LocalModelPool>,
        models_dir: PathBuf,
        source_lang: &str,
        target_lang: &str,
        lease: crate::integrations::models::ModelLease<sona_translation_onnx::OnnxNmtAdapter>,
    ) -> Self {
        let src =
            crate::integrations::translation::normalize_language_code(source_lang).to_string();
        let tgt =
            crate::integrations::translation::normalize_language_code(target_lang).to_string();
        Self {
            pool,
            models_dir,
            lease: parking_lot::Mutex::new(Some(((src, tgt), lease))),
        }
    }
}

#[async_trait]
impl TranslationEnginePort for PooledOnnxTranslationEngine {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        let norm_src = crate::integrations::translation::normalize_language_code(source_lang);
        let norm_tgt = crate::integrations::translation::normalize_language_code(target_lang);

        let cached_adapter = {
            let guard = self.lease.lock();
            if let Some(((curr_src, curr_tgt), lease)) = guard.as_ref()
                && curr_src == norm_src
                && curr_tgt == norm_tgt
            {
                Some(lease.model_arc())
            } else {
                None
            }
        };

        let adapter = if let Some(adapter) = cached_adapter {
            adapter
        } else {
            let pool = Arc::clone(&self.pool);
            let models_dir = self.models_dir.clone();
            let src_string = norm_src.to_string();
            let tgt_string = norm_tgt.to_string();

            let new_lease = tokio::task::spawn_blocking(move || {
                pool.acquire_onnx_model(&src_string, &tgt_string, &models_dir)
            })
            .await
            .map_err(|e| TranslationPortError::engine_failed(e.to_string()))??;

            let model_arc = new_lease.model_arc();
            let mut guard = self.lease.lock();
            *guard = Some(((norm_src.to_string(), norm_tgt.to_string()), new_lease));
            model_arc
        };

        adapter.translate(text, source_lang, target_lang).await
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::OnnxNmt
    }
}

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
                let lease = services.local_model_pool.acquire_onnx_model(
                    source_lang,
                    target_lang,
                    models_dir,
                )?;

                let pooled_engine = Arc::new(PooledOnnxTranslationEngine::new(
                    Arc::clone(&services.local_model_pool),
                    models_dir.to_path_buf(),
                    source_lang,
                    target_lang,
                    lease,
                ));

                Ok(Arc::new(SemaphoreGuardedTranslationEngine {
                    inner: pooled_engine,
                    semaphore: Arc::clone(&services.local_nmt_semaphore),
                }))
            }
            TranslationEngineKind::LlamaCpp => {
                let model_name = match model_name_or_id {
                    Some(m) if !m.trim().is_empty() => m.trim(),
                    _ => "",
                };
                let engine = Arc::new(
                    sona_llama_cpp::LlamaCppLlmEngine::with_models_dir_and_provider(
                        Some(models_dir.to_path_buf()),
                        Some(Arc::clone(&services.local_model_pool)
                            as Arc<dyn sona_llama_cpp::LlamaModelProvider>),
                    ),
                );
                let adapter = Arc::new(
                    sona_llama_cpp::translation::LlamaCppTranslationAdapter::new(
                        engine, model_name,
                    ),
                );
                Ok(Arc::new(SemaphoreGuardedTranslationEngine {
                    inner: adapter,
                    semaphore: Arc::clone(&services.local_llm_semaphore),
                }))
            }
            TranslationEngineKind::Online => {
                let mut config = sona_core::llm::requests::LlmConfig::default();
                if let Some(id) = model_name_or_id
                    && !id.trim().is_empty()
                {
                    config.model = id.to_string();
                }
                Ok(Self::create_online_engine(services, config))
            }
        }
    }

    pub fn create_online_engine(
        services: &DesktopServices,
        config: sona_core::llm::requests::LlmConfig,
    ) -> Arc<dyn TranslationEnginePort> {
        let models_dir = Some(services.active_models_dir());
        let cache_dir = Some(services.active_data_dir().join("cache"));
        let desktop_llm = crate::integrations::llm::DesktopLlmAdapter::with_services(
            models_dir,
            cache_dir,
            Some(services),
        );
        Arc::new(
            crate::integrations::translation::OnlineStreamingTranslationAdapter::new(
                desktop_llm,
                config,
            ),
        )
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

    #[test]
    fn factory_creates_online_engine_successfully() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = create_test_services(temp_dir.path());
        let engine = TranslationEngineFactory::create_engine(
            &services,
            TranslationEngineKind::Online,
            Some("gpt-4o"),
            "en",
            "zh",
            temp_dir.path(),
        )
        .unwrap();

        assert_eq!(engine.engine_kind(), TranslationEngineKind::Online);
    }
}
