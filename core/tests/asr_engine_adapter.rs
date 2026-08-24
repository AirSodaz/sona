use std::sync::Arc;

use async_trait::async_trait;
use sona_core::export::ExportFormat;
use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamingSession, BatchTranscriberPort,
    EngineCapabilities, LocalAsrAdapter, LocalAsrEngine, StreamingAsrFactoryPort,
    StreamingInferenceSpec,
};
use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
use sona_core::transcription::transcript::TranscriptSegment;

#[derive(Default)]
struct FakeTranscriber {
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl BatchTranscriberPort for FakeTranscriber {
    async fn transcribe(
        &self,
        _plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Vec::new())
    }
}

struct FakeStreamingFactory;

#[async_trait]
impl StreamingAsrFactoryPort for FakeStreamingFactory {
    async fn prepare(&self, _spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn create(
        &self,
        _pipeline_id: &str,
        _spec: &StreamingInferenceSpec,
        _observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        Ok(Arc::new(FakeSession))
    }
}

struct FakeSession;

#[async_trait]
impl AsrStreamingSession for FakeSession {
    async fn start(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn flush(&self) -> Result<(), AsrPortError> {
        Ok(())
    }

    async fn feed_audio_frame(&self, _frame: AsrAudioFrame) -> Result<(), AsrPortError> {
        Ok(())
    }
}

/// A fully synthetic engine: proves `LocalAsrAdapter` carries no sherpa- or
/// llama-specific assumptions and can be implemented against Core alone.
struct FakeEngineAdapter;

impl LocalAsrAdapter for FakeEngineAdapter {
    fn engine(&self) -> LocalAsrEngine {
        LocalAsrEngine::SherpaOnnx
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::BATCH
    }

    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
        Arc::new(FakeTranscriber::default())
    }

    fn streaming_factory(&self) -> Option<Arc<dyn StreamingAsrFactoryPort>> {
        None
    }
}

fn demo_plan() -> BatchTranscribePlan {
    BatchTranscribePlan {
        input_path: "audio.wav".into(),
        save_to_path: None,
        engine: LocalAsrEngine::SherpaOnnx,
        model_path: "models/demo".to_string(),
        num_threads: 1,
        enable_itn: false,
        language: "auto".to_string(),
        punctuation_model: None,
        vad_model: None,
        vad_buffer: 5.0,
        batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
        model_type: "whisper".to_string(),
        file_config: None,
        hotwords: None,
        speaker_processing: None,
        gpu_acceleration: None,
        export_format: ExportFormat::Json,
        output_target: OutputTarget::Stdout,
        quiet: true,
    }
}

#[tokio::test]
async fn adapter_routes_batch_through_its_port() {
    let adapter = FakeEngineAdapter;
    assert_eq!(adapter.engine(), LocalAsrEngine::SherpaOnnx);
    assert!(adapter.capabilities().contains(EngineCapabilities::BATCH));
    assert!(!adapter.capabilities().contains(EngineCapabilities::STREAMING));

    let segments = adapter.batch_transcriber().transcribe(demo_plan()).await.unwrap();
    assert!(segments.is_empty());
}

#[test]
fn adapter_without_streaming_reports_no_factory() {
    let adapter = FakeEngineAdapter;
    assert!(adapter.streaming_factory().is_none());
}

/// Second synthetic engine declaring live support: guards the `STREAMING`
/// bit ⇔ factory agreement from the capable side.
struct StreamingCapableAdapter;

impl LocalAsrAdapter for StreamingCapableAdapter {
    fn engine(&self) -> LocalAsrEngine {
        LocalAsrEngine::SherpaOnnx
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::BATCH | EngineCapabilities::STREAMING
    }

    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
        Arc::new(FakeTranscriber::default())
    }

    fn streaming_factory(&self) -> Option<Arc<dyn StreamingAsrFactoryPort>> {
        Some(Arc::new(FakeStreamingFactory))
    }
}

#[tokio::test]
async fn streaming_capable_adapter_creates_sessions_through_its_factory() {
    let adapter = StreamingCapableAdapter;
    let factory = adapter
        .streaming_factory()
        .expect("streaming-capable adapters must expose a factory");

    let request = demo_streaming_request();
    let spec = StreamingInferenceSpec::from_request(&request).unwrap();

    factory.prepare(&spec).await.unwrap();
    let session = factory
        .create(
            "pipeline-1",
            &spec,
            Arc::new(sona_core::ports::asr::NoopAsrRuntimeObserver),
        )
        .await
        .unwrap();
    session
        .feed_audio_frame(AsrAudioFrame::new(0, 0, vec![0.0f32; 16]))
        .await
        .unwrap();
    session.flush().await.unwrap();
    session.stop().await.unwrap();
}

fn demo_streaming_request() -> sona_core::ports::asr::AsrTranscriptionRequest {
    sona_core::ports::asr::AsrTranscriptionRequest::local_sherpa(
        sona_core::ports::asr::AsrMode::Streaming,
        "models/demo".to_string(),
        1,
        false,
        "auto".to_string(),
        None,
        None,
        5.0,
        "whisper".to_string(),
        None,
        None,
        Default::default(),
        Default::default(),
        None,
        None,
    )
}

#[test]
fn capability_flags_are_independent_bits() {
    let all = EngineCapabilities::BATCH
        | EngineCapabilities::STREAMING
        | EngineCapabilities::SPEAKER
        | EngineCapabilities::PUNCTUATION
        | EngineCapabilities::HOTWORDS
        | EngineCapabilities::GPU;
    assert_eq!(all.bits().count_ones(), 6);
}
