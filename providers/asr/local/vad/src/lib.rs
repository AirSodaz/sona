//! Silero ONNX voice activity detection behind the Core `VadEnginePort`.
//!
//! The crate ships every built-in VAD engine and is composed by hosts through
//! [`built_in_engines`] so all local ASR adapters share speech segmentation
//! without depending on sherpa-onnx themselves.

mod shared;
mod silero;
mod ten;

pub use shared::resolve_model_onnx_path;
pub use silero::SileroOnnxEngine;
pub use ten::TenVadOnnxEngine;

use sona_core::ports::vad::VadEngineSet;
use std::sync::Arc;

/// Every built-in VAD engine, in dispatch order.
///
/// Engines are consulted in registration order; unknown model files fall back
/// to the Silero engine so custom directories keep working.
pub fn built_in_engines() -> VadEngineSet {
    VadEngineSet::empty()
        .register(Arc::new(SileroOnnxEngine))
        .register(Arc::new(TenVadOnnxEngine))
}
