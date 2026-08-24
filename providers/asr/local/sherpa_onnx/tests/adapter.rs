use sona_core::ports::asr::{EngineCapabilities, LocalAsrAdapter, LocalAsrEngine};
use sona_sherpa_onnx::SherpaOnnxAdapter;

#[test]
fn sherpa_adapter_declares_full_capability_set() {
    let adapter = SherpaOnnxAdapter::default();

    assert_eq!(adapter.engine(), LocalAsrEngine::SherpaOnnx);
    let expected = EngineCapabilities::BATCH
        | EngineCapabilities::STREAMING
        | EngineCapabilities::SPEAKER
        | EngineCapabilities::PUNCTUATION
        | EngineCapabilities::HOTWORDS
        | EngineCapabilities::GPU;
    assert_eq!(adapter.capabilities(), expected);
}

#[test]
fn sherpa_adapter_exposes_streaming_factory() {
    let adapter = SherpaOnnxAdapter::default();
    assert!(adapter.streaming_factory().is_some());
}
