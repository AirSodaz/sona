use std::sync::Arc;

use sona_core::ports::asr::{
    BatchTranscriberPort, EngineCapabilities, LocalAsrAdapter, LocalAsrEngine,
    StreamingAsrFactoryPort,
};
use sona_core::ports::vad::VadEngineSet;

use crate::batch::LlamaBatchAsrAdapter;

/// Provider facade for the llama.cpp local ASR engine (Qwen3-ASR and
/// Granite Speech batch inference).
///
/// The engine supports file transcription only; live streaming sessions
/// remain exclusive to engines that declare the `STREAMING` capability.
#[derive(Clone, Default)]
pub struct LlamaCppAdapter {
    vad_engines: VadEngineSet,
}

impl LlamaCppAdapter {
    pub fn new(vad_engines: VadEngineSet) -> Self {
        Self { vad_engines }
    }
}

impl LocalAsrAdapter for LlamaCppAdapter {
    fn engine(&self) -> LocalAsrEngine {
        LocalAsrEngine::LlamaCpp
    }

    fn capabilities(&self) -> EngineCapabilities {
        // Both supported models consume hotwords through their trained
        // prompt mechanisms (Qwen3 system-message context; Granite Speech
        // `Keywords:` suffixes).
        EngineCapabilities::BATCH | EngineCapabilities::HOTWORDS
    }

    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
        Arc::new(LlamaBatchAsrAdapter::new(self.vad_engines.clone()))
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
        let adapter = LlamaCppAdapter::default();

        assert_eq!(adapter.engine(), LocalAsrEngine::LlamaCpp);
        assert_eq!(
            adapter.capabilities(),
            EngineCapabilities::BATCH | EngineCapabilities::HOTWORDS
        );
        assert!(adapter.streaming_factory().is_none());
    }
}
