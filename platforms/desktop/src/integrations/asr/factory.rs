pub use sona_application::local_asr::HybridStreamingAsrFactory;
use sona_application::local_asr::LocalAsrRegistry;
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::sync::Arc;

/// Builds a streaming ASR factory by composing the shared application
/// hybrid factory with provider adapters and automatic idle resource pruners.
pub fn create_hybrid_streaming_asr_factory(
    registry: LocalAsrRegistry,
) -> HybridStreamingAsrFactory {
    HybridStreamingAsrFactory::from_local_registry(
        registry,
        Some(Arc::new(sona_online_asr::OnlineAsrAdapter)),
    )
}

/// Builds a desktop streaming ASR factory by composing the shared application
/// hybrid factory with desktop provider adapters and automatic idle resource pruners.
pub fn create_desktop_streaming_asr_factory(
    registry: LocalAsrRegistry,
    _recognizer_pool: RecognizerPool,
) -> HybridStreamingAsrFactory {
    create_hybrid_streaming_asr_factory(registry)
}

/// Desktop composition root for streaming ASR.
pub type DesktopStreamingAsrFactory = HybridStreamingAsrFactory;
