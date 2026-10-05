use std::sync::Arc;

use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrEngine, AsrPortError, AsrPortErrorKind, AsrRuntimeObserver, AsrStreamingSession,
    BatchTranscriberPort, BatchTranscriptionObserver, EngineCapabilities, LocalAsrAdapter,
    LocalAsrEngine, NoopAsrRuntimeObserver, StreamingAsrFactoryPort, StreamingInferenceSpec,
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

#[async_trait]
impl IdleResourcePruner for LocalAsrRegistry {
    async fn prune_idle_resources(&self) {
        for adapter in &self.adapters {
            adapter.prune_idle_resources().await;
        }
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

/// Routes streaming transcription to the local engine selected in each spec.
#[derive(Clone)]
pub struct LocalStreamingAsrFactory {
    registry: LocalAsrRegistry,
}

impl LocalStreamingAsrFactory {
    pub fn new(registry: LocalAsrRegistry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &LocalAsrRegistry {
        &self.registry
    }

    fn local_streaming_factory(
        &self,
        spec: &StreamingInferenceSpec,
    ) -> Result<Arc<dyn StreamingAsrFactoryPort>, AsrPortError> {
        let request = spec.engine_request();
        let engine = request.engine_config.local_engine().ok_or_else(|| {
            AsrPortError::invalid_request("Local streaming requires a local engine selection")
        })?;
        let adapter = self.registry.get(engine).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "The {} local ASR engine is not available on this host.",
                    engine.as_str()
                ),
            )
        })?;
        adapter.streaming_factory().ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "The {} local ASR engine does not support streaming transcription.",
                    engine.as_str()
                ),
            )
        })
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for LocalStreamingAsrFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        let factory = self.local_streaming_factory(spec)?;
        factory.prepare(spec).await
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        let factory = self.local_streaming_factory(spec)?;
        factory.create(pipeline_id, spec, observer).await
    }
}

/// Idle resource cleanup hook invoked when switching to online ASR.
#[async_trait]
pub trait IdleResourcePruner: Send + Sync {
    async fn prune_idle_resources(&self);
}

#[async_trait]
impl<F, Fut> IdleResourcePruner for F
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = ()> + Send,
{
    async fn prune_idle_resources(&self) {
        (self)().await;
    }
}

/// Routes streaming transcription across local and online ASR engines.
///
/// Dispatches `AsrEngine::Local` to an underlying local streaming factory
/// (such as [`LocalStreamingAsrFactory`]), and `AsrEngine::Online` to an online
/// streaming factory port, invoking an optional [`IdleResourcePruner`] to
/// release idle resources (e.g. idle local model weights) before starting
/// online inference.
#[derive(Clone)]
pub struct HybridStreamingAsrFactory {
    local: Arc<dyn StreamingAsrFactoryPort>,
    online: Option<Arc<dyn StreamingAsrFactoryPort>>,
    idle_pruner: Option<Arc<dyn IdleResourcePruner>>,
}

pub type RoutedStreamingAsrFactory = HybridStreamingAsrFactory;

impl HybridStreamingAsrFactory {
    pub fn new(
        local: Arc<dyn StreamingAsrFactoryPort>,
        online: Option<Arc<dyn StreamingAsrFactoryPort>>,
    ) -> Self {
        Self {
            local,
            online,
            idle_pruner: None,
        }
    }

    pub fn from_local_registry(
        registry: LocalAsrRegistry,
        online: Option<Arc<dyn StreamingAsrFactoryPort>>,
    ) -> Self {
        let pruner: Arc<dyn IdleResourcePruner> = Arc::new(registry.clone());
        Self::new(Arc::new(LocalStreamingAsrFactory::new(registry)), online)
            .with_idle_pruner(pruner)
    }

    pub fn with_idle_pruner(mut self, pruner: Arc<dyn IdleResourcePruner>) -> Self {
        self.idle_pruner = Some(pruner);
        self
    }

    pub fn coordinator(&self) -> crate::live_transcription::LiveTranscriptionCoordinator {
        crate::live_transcription::LiveTranscriptionCoordinator::new(
            Arc::new(self.clone()),
            Arc::new(NoopAsrRuntimeObserver),
        )
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for HybridStreamingAsrFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        match spec.engine() {
            AsrEngine::Local => self.local.prepare(spec).await,
            AsrEngine::Online => {
                let online = self.online.as_ref().ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::Unsupported,
                        "Online ASR streaming is not configured or supported on this host.",
                    )
                })?;
                online.prepare(spec).await?;
                if let Some(pruner) = &self.idle_pruner {
                    pruner.prune_idle_resources().await;
                }
                Ok(())
            }
        }
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        match spec.engine() {
            AsrEngine::Local => self.local.create(pipeline_id, spec, observer).await,
            AsrEngine::Online => {
                let online = self.online.as_ref().ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::Unsupported,
                        "Online ASR streaming is not configured or supported on this host.",
                    )
                })?;
                if let Some(pruner) = &self.idle_pruner {
                    pruner.prune_idle_resources().await;
                }
                online.create(pipeline_id, spec, observer).await
            }
        }
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

    struct CountingStreamingFactory {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl StreamingAsrFactoryPort for CountingStreamingFactory {
        async fn prepare(&self, _spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn create(
            &self,
            _pipeline_id: &str,
            _spec: &StreamingInferenceSpec,
            _observer: Arc<dyn AsrRuntimeObserver>,
        ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(AsrPortError::runtime("mock session creation"))
        }
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
            if self.streaming {
                Some(Arc::new(CountingStreamingFactory {
                    calls: self.calls.clone(),
                }))
            } else {
                None
            }
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
            alignment_model: None,
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
            ffmpeg_path: None,
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
    fn streaming_spec(local_engine: LocalAsrEngine) -> StreamingInferenceSpec {
        let request = sona_core::ports::asr::AsrTranscriptionRequest {
            mode: sona_core::ports::asr::AsrMode::Streaming,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: sona_core::ports::asr::AsrEngineConfig::Local {
                local_engine,
                model_id: None,
                model_path: "models/demo".to_string(),
                num_threads: 4,
                punctuation_model: None,
                alignment_model: None,
                vad_model: None,
                vad_buffer: 5.0,
                batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
                model_type: "sense-voice".to_string(),
                file_config: Box::new(None),
                gpu_acceleration: None,
                initial_refresh_rate_ms: None,
                enable_partial_decoding: None,
                ffmpeg_path: None,
            },
        };
        StreamingInferenceSpec::from_request(&request).unwrap()
    }

    #[tokio::test]
    async fn local_streaming_factory_routes_to_streaming_engine() {
        let (registry, sherpa_calls, _) = two_engine_registry();
        let factory = LocalStreamingAsrFactory::new(registry);
        let spec = streaming_spec(LocalAsrEngine::SherpaOnnx);

        factory.prepare(&spec).await.unwrap();
        assert_eq!(sherpa_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn local_streaming_factory_rejects_non_streaming_engine() {
        let (registry, _, _) = two_engine_registry();
        let factory = LocalStreamingAsrFactory::new(registry);
        let spec = streaming_spec(LocalAsrEngine::LlamaCpp);

        let err = factory.prepare(&spec).await.unwrap_err();
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);
    }

    fn online_streaming_spec() -> StreamingInferenceSpec {
        let request = sona_core::ports::asr::AsrTranscriptionRequest {
            engine_config: sona_core::ports::asr::AsrEngineConfig::Online {
                provider: sona_core::ports::asr::OnlineAsrProviderRequest {
                    provider_id: "test-provider".to_string(),
                    profile_id: "test-profile".to_string(),
                    config: serde_json::json!({}),
                },
            },
            mode: sona_core::ports::asr::AsrMode::Streaming,
            enable_itn: false,
            language: "zh".to_string(),
            hotwords: None,
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        };
        StreamingInferenceSpec::from_request(&request).unwrap()
    }

    #[tokio::test]
    async fn hybrid_streaming_factory_routes_local_to_local_factory() {
        let (registry, sherpa_calls, _) = two_engine_registry();
        let factory = HybridStreamingAsrFactory::from_local_registry(registry, None);
        let spec = streaming_spec(LocalAsrEngine::SherpaOnnx);

        factory.prepare(&spec).await.unwrap();
        assert_eq!(sherpa_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn hybrid_streaming_factory_routes_online_and_invokes_pruner() {
        let (registry, _, _) = two_engine_registry();
        let online_calls = Arc::new(AtomicUsize::new(0));
        let prune_calls = Arc::new(AtomicUsize::new(0));

        let online_factory: Arc<dyn StreamingAsrFactoryPort> = Arc::new(CountingStreamingFactory {
            calls: online_calls.clone(),
        });
        let pruner_counter = prune_calls.clone();
        let factory =
            HybridStreamingAsrFactory::from_local_registry(registry, Some(online_factory))
                .with_idle_pruner(Arc::new(move || {
                    let counter = pruner_counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                }));

        let spec = online_streaming_spec();
        factory.prepare(&spec).await.unwrap();
        assert_eq!(online_calls.load(Ordering::SeqCst), 1);
        assert_eq!(prune_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn hybrid_streaming_factory_rejects_missing_online_factory() {
        let (registry, _, _) = two_engine_registry();
        let factory = HybridStreamingAsrFactory::from_local_registry(registry, None);
        let spec = online_streaming_spec();

        let err = factory.prepare(&spec).await.unwrap_err();
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn hybrid_streaming_factory_create_routes_online_and_invokes_pruner() {
        let (registry, _, _) = two_engine_registry();
        let online_calls = Arc::new(AtomicUsize::new(0));
        let prune_calls = Arc::new(AtomicUsize::new(0));

        let online_factory: Arc<dyn StreamingAsrFactoryPort> = Arc::new(CountingStreamingFactory {
            calls: online_calls.clone(),
        });
        let pruner_counter = prune_calls.clone();
        let factory =
            HybridStreamingAsrFactory::from_local_registry(registry, Some(online_factory))
                .with_idle_pruner(Arc::new(move || {
                    let counter = pruner_counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                }));

        let spec = online_streaming_spec();
        let _ = factory
            .create("test-pipe", &spec, Arc::new(NoopAsrRuntimeObserver))
            .await;
        assert_eq!(online_calls.load(Ordering::SeqCst), 1);
        assert_eq!(prune_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn hybrid_streaming_factory_create_rejects_missing_online_without_pruning() {
        let (registry, _, _) = two_engine_registry();
        let prune_calls = Arc::new(AtomicUsize::new(0));
        let pruner_counter = prune_calls.clone();
        let factory = HybridStreamingAsrFactory::from_local_registry(registry, None)
            .with_idle_pruner(Arc::new(move || {
                let counter = pruner_counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }));

        let spec = online_streaming_spec();
        let err = match factory
            .create("test-pipe", &spec, Arc::new(NoopAsrRuntimeObserver))
            .await
        {
            Err(e) => e,
            Ok(_) => panic!("expected create to fail without online factory"),
        };
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);
        assert_eq!(
            prune_calls.load(Ordering::SeqCst),
            0,
            "idle pruner must not be invoked when online factory is absent"
        );
    }
}
