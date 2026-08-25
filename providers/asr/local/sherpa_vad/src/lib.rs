//! Silero ONNX voice activity detection behind the Core `VadEnginePort`.
//!
//! The engine is composed by hosts into a `VadEngineSet` so every local ASR
//! adapter can share speech segmentation without depending on sherpa-onnx
//! itself.

mod engine;

pub use engine::SherpaVadEngine;
