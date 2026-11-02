//! Voice activity detection provider for Sona.
//!
//! Provides native ONNX Runtime implementations for Silero VAD (v4/v5/v6)
//! and Alibaba FunASR FSMN-VAD with integrated 80-dim log-mel DSP frontend.

pub mod engine;
pub mod fbank;
pub mod runners;
pub mod shared;
pub mod state_machine;
pub mod streaming;

pub use engine::{FsmnVadOnnxEngine, SileroOnnxEngine};
pub use shared::resolve_model_onnx_path;
pub use streaming::NativeStreamingVad;

use sona_core::ports::vad::VadEngineSet;
use std::sync::Arc;

/// Every built-in VAD engine, in dispatch order.
///
/// Engines are consulted in registration order: `FsmnVadOnnxEngine` claims models
/// matching "fsmn", while `SileroOnnxEngine` handles Silero models and acts as the
/// universal fallback for custom ONNX models.
pub fn built_in_engines() -> VadEngineSet {
    VadEngineSet::empty()
        .register(Arc::new(SileroOnnxEngine))
        .register(Arc::new(FsmnVadOnnxEngine))
}
