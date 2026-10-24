//! Transcript punctuation behind the Core `PunctuationEnginePort`.
//!
//! The crate ships every built-in punctuation engine and is composed by hosts
//! through [`built_in_engines`] so any local ASR adapter can add punctuation
//! without depending on sherpa-onnx itself.

mod cnn_bilstm;
mod ct_transformer;
mod shared;

pub use cnn_bilstm::CnnBiLstmEngine;
pub use ct_transformer::CtTransformerEngine;
pub use shared::resolve_model_onnx_path;

use sona_core::ports::punctuation::PunctuationEngineSet;
use std::sync::Arc;

/// Every built-in punctuation engine, in dispatch order.
pub fn built_in_engines() -> PunctuationEngineSet {
    PunctuationEngineSet::empty()
        .register(Arc::new(CnnBiLstmEngine))
        .register(Arc::new(CtTransformerEngine))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::punctuation::PunctuationEngineKind;
    use std::fs;

    #[test]
    fn built_in_engines_dispatch_order() {
        let engines = built_in_engines();

        let root = std::env::temp_dir().join(format!("sona-dispatch-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        // Empty dir or dir with only onnx resolves to CtTransformer
        let onnx_path = root.join("model.onnx");
        fs::write(&onnx_path, b"dummy onnx").unwrap();
        let engine = engines.resolve(Some(&root)).expect("engine found");
        assert_eq!(
            engine.engine_kind(),
            PunctuationEngineKind::CtTransformerOnnx
        );

        // Dir with both onnx and bpe.vocab resolves to CnnBiLstm
        let vocab_path = root.join("bpe.vocab");
        fs::write(&vocab_path, b"dummy vocab").unwrap();
        let engine = engines.resolve(Some(&root)).expect("engine found");
        assert_eq!(engine.engine_kind(), PunctuationEngineKind::CnnBiLstmOnnx);

        let _ = fs::remove_dir_all(&root);
    }
}
