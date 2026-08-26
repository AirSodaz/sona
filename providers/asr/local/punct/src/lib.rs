//! Transcript punctuation behind the Core `PunctuationEnginePort`.
//!
//! The crate ships every built-in punctuation engine and is composed by hosts
//! through [`built_in_engines`] so any local ASR adapter can add punctuation
//! without depending on sherpa-onnx itself.

mod ct_transformer;
mod shared;

pub use ct_transformer::CtTransformerEngine;
pub use shared::resolve_model_onnx_path;

use sona_core::ports::punctuation::PunctuationEngineSet;
use std::sync::Arc;

/// Every built-in punctuation engine, in dispatch order.
pub fn built_in_engines() -> PunctuationEngineSet {
    PunctuationEngineSet::empty().register(Arc::new(CtTransformerEngine))
}
