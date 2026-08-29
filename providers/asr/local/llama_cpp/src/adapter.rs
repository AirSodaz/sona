use std::sync::Arc;

use sona_core::ports::asr::{
    BatchTranscriberPort, EngineCapabilities, LocalAsrAdapter, LocalAsrEngine,
    StreamingAsrFactoryPort,
};
use sona_core::ports::punctuation::PunctuationEngineSet;
use sona_core::ports::vad::VadEngineSet;

use crate::batch::{LlamaBatchAsrAdapter, gpu_backend_available};

/// Provider facade for the llama.cpp local ASR engine (Qwen3-ASR batch inference).
///
/// The engine supports file transcription only; live streaming sessions
/// remain exclusive to engines that declare the `STREAMING` capability.
#[derive(Clone, Default)]
pub struct LlamaCppAdapter {
    vad_engines: VadEngineSet,
    punct_engines: PunctuationEngineSet,
}

impl LlamaCppAdapter {
    pub fn new(vad_engines: VadEngineSet, punct_engines: PunctuationEngineSet) -> Self {
        Self {
            vad_engines,
            punct_engines,
        }
    }
}

impl LocalAsrAdapter for LlamaCppAdapter {
    fn engine(&self) -> LocalAsrEngine {
        LocalAsrEngine::LlamaCpp
    }

    fn capabilities(&self) -> EngineCapabilities {
        // Qwen3-ASR consumes hotwords through its trained system-message
        // context. The GPU bit reflects the backends the linked ggml
        // runtime actually registered at runtime.
        let mut capabilities = EngineCapabilities::BATCH | EngineCapabilities::HOTWORDS;
        if gpu_backend_available() {
            capabilities |= EngineCapabilities::GPU;
        }
        capabilities
    }

    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
        Arc::new(LlamaBatchAsrAdapter::new(
            self.vad_engines.clone(),
            self.punct_engines.clone(),
        ))
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
        let mut expected = EngineCapabilities::BATCH | EngineCapabilities::HOTWORDS;
        if gpu_backend_available() {
            expected |= EngineCapabilities::GPU;
        }
        assert_eq!(adapter.capabilities(), expected);
        assert!(adapter.streaming_factory().is_none());
    }
}
