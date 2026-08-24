use std::sync::Arc;

use sona_core::ports::asr::{
    BatchTranscriberPort, EngineCapabilities, LocalAsrAdapter, LocalAsrEngine,
    StreamingAsrFactoryPort,
};

use crate::batch::LlamaBatchAsrAdapter;

/// Provider facade for the llama.cpp local ASR engine (Qwen3-ASR batch
/// inference).
///
/// The engine currently supports file transcription only; live streaming
/// sessions remain exclusive to engines that declare the `STREAMING`
/// capability.
pub struct LlamaCppAdapter;

impl LocalAsrAdapter for LlamaCppAdapter {
    fn engine(&self) -> LocalAsrEngine {
        LocalAsrEngine::LlamaCpp
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::BATCH
    }

    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
        Arc::new(LlamaBatchAsrAdapter)
    }

    fn streaming_factory(&self) -> Option<Arc<dyn StreamingAsrFactoryPort>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama_adapter_is_batch_only() {
        let adapter = LlamaCppAdapter;

        assert_eq!(adapter.engine(), LocalAsrEngine::LlamaCpp);
        assert_eq!(adapter.capabilities(), EngineCapabilities::BATCH);
        assert!(adapter.streaming_factory().is_none());
    }
}
