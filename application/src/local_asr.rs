use std::sync::Arc;

use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, BatchTranscriberPort, BatchTranscriptionObserver,
    EngineCapabilities, LocalAsrAdapter, LocalAsrEngine,
};
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::transcript::TranscriptSegment;

/// What an engine can do, exposed for feature gating and UI availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineInfo {
    pub engine: LocalAsrEngine,
    pub capabilities: EngineCapabilities,
}

/// Composition-time registry of the local ASR engines available on this
/// host.
///
/// Hosts build one registry at startup by registering provider adapters;
/// everything downstream (batch routing, streaming factories, UI
/// availability) reads from it instead of importing concrete engines.
#[derive(Clone, Default)]
pub struct LocalAsrRegistry {
    adapters: Vec<Arc<dyn LocalAsrAdapter>>,
}

impl LocalAsrRegistry {
    pub fn empty() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Builder-style registration for composition roots.
    pub fn register(mut self, adapter: Arc<dyn LocalAsrAdapter>) -> Self {
        self.adapters.push(adapter);
        self
    }

    pub fn get(&self, engine: LocalAsrEngine) -> Option<Arc<dyn LocalAsrAdapter>> {
        self.adapters
            .iter()
            .find(|adapter| adapter.engine() == engine)
            .cloned()
    }

    pub fn available(&self) -> Vec<EngineInfo> {
        self.adapters
            .iter()
            .map(|adapter| EngineInfo {
                engine: adapter.engine(),
                capabilities: adapter.capabilities(),
            })
            .collect()
    }
}

/// Routes batch transcription to the engine selected in each plan.
///
/// Observers are forwarded to the selected adapter so engines keep their
/// incremental progress behavior.
#[derive(Clone)]
pub struct LocalBatchTranscriberRouter {
    registry: LocalAsrRegistry,
}

impl LocalBatchTranscriberRouter {
    pub fn new(registry: LocalAsrRegistry) -> Self {
        Self { registry }
    }

    fn batch_transcriber(
        &self,
        engine: LocalAsrEngine,
    ) -> Result<Arc<dyn BatchTranscriberPort>, AsrPortError> {
        let adapter = self.registry.get(engine).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "The {} local ASR engine is not available on this host.",
                    engine.as_str()
                ),
            )
        })?;
        Ok(adapter.batch_transcriber())
    }
}

#[async_trait]
impl BatchTranscriberPort for LocalBatchTranscriberRouter {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let engine = plan.engine;
        self.batch_transcriber(engine)?.transcribe(plan).await
    }

    async fn transcribe_with_observer(
        &self,
        plan: BatchTranscribePlan,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let engine = plan.engine;
        self.batch_transcriber(engine)?
            .transcribe_with_observer(plan, observer)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::export::ExportFormat;
    use sona_core::ports::asr::StreamingAsrFactoryPort;
    use sona_core::transcription::runtime::OutputTarget;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingTranscriber {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl BatchTranscriberPort for CountingTranscriber {
        async fn transcribe(
            &self,
            _plan: BatchTranscribePlan,
        ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    struct FakeAdapter {
        engine: LocalAsrEngine,
        calls: Arc<AtomicUsize>,
        streaming: bool,
        hotwords: bool,
    }

    impl LocalAsrAdapter for FakeAdapter {
        fn engine(&self) -> LocalAsrEngine {
            self.engine
        }

        fn capabilities(&self) -> EngineCapabilities {
            let mut capabilities = if self.streaming {
                EngineCapabilities::BATCH | EngineCapabilities::STREAMING
            } else {
                EngineCapabilities::BATCH
            };
            if self.hotwords {
                capabilities |= EngineCapabilities::HOTWORDS;
            }
            capabilities
        }

        fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
            Arc::new(CountingTranscriber {
                calls: self.calls.clone(),
            })
        }

        fn streaming_factory(&self) -> Option<Arc<dyn StreamingAsrFactoryPort>> {
            None
        }
    }

    impl FakeAdapter {
        fn sherpa(calls: Arc<AtomicUsize>) -> Self {
            Self {
                engine: LocalAsrEngine::SherpaOnnx,
                calls,
                streaming: true,
                hotwords: true,
            }
        }

        fn llama(calls: Arc<AtomicUsize>) -> Self {
            Self {
                engine: LocalAsrEngine::LlamaCpp,
                calls,
                streaming: false,
                hotwords: true,
            }
        }
    }

    fn plan(engine: LocalAsrEngine) -> BatchTranscribePlan {
        BatchTranscribePlan {
            input_path: PathBuf::from("audio.wav"),
            save_to_path: None,
            engine,
            model_path: "models/demo".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
            model_type: "qwen3-asr".to_string(),
            file_config: None,
            hotwords: None,
            speaker_processing: None,
            gpu_acceleration: None,
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        }
    }

    fn two_engine_registry() -> (LocalAsrRegistry, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let sherpa_calls = Arc::new(AtomicUsize::new(0));
        let llama_calls = Arc::new(AtomicUsize::new(0));
        let registry = LocalAsrRegistry::empty()
            .register(Arc::new(FakeAdapter::sherpa(sherpa_calls.clone())))
            .register(Arc::new(FakeAdapter::llama(llama_calls.clone())));
        (registry, sherpa_calls, llama_calls)
    }

    #[tokio::test]
    async fn routes_each_local_engine_to_its_adapter() {
        let (registry, sherpa_calls, llama_calls) = two_engine_registry();
        let router = LocalBatchTranscriberRouter::new(registry);

        router
            .transcribe(plan(LocalAsrEngine::SherpaOnnx))
            .await
            .unwrap();
        router
            .transcribe(plan(LocalAsrEngine::LlamaCpp))
            .await
            .unwrap();

        assert_eq!(sherpa_calls.load(Ordering::SeqCst), 1);
        assert_eq!(llama_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reports_unavailable_engines_as_unsupported() {
        // Simulate a host that only ships sherpa.
        let sherpa_only = LocalAsrRegistry::empty()
            .register(Arc::new(FakeAdapter::sherpa(Arc::new(AtomicUsize::new(0)))));
        let router = LocalBatchTranscriberRouter::new(sherpa_only);

        let error = router
            .transcribe(plan(LocalAsrEngine::LlamaCpp))
            .await
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    }

    #[test]
    fn available_reports_registered_capabilities() {
        let (registry, _, _) = two_engine_registry();
        let mut infos = registry.available();
        infos.sort_by_key(|info| info.engine.as_str());

        assert_eq!(infos.len(), 2);
        let llama_info = infos
            .iter()
            .find(|i| i.engine == LocalAsrEngine::LlamaCpp)
            .unwrap();
        assert_eq!(
            llama_info.capabilities,
            EngineCapabilities::BATCH | EngineCapabilities::HOTWORDS
        );
        let sherpa_info = infos
            .iter()
            .find(|i| i.engine == LocalAsrEngine::SherpaOnnx)
            .unwrap();
        assert!(
            sherpa_info
                .capabilities
                .contains(EngineCapabilities::STREAMING)
        );
    }
}
