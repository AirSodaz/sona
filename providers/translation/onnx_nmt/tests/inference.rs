use sona_core::ports::translation::{TranslationEngineKind, TranslationEnginePort};
use sona_translation_onnx::{OnnxNmtAdapter, OnnxNmtModel, SentencePieceTokenizer};
use std::path::Path;
use std::time::Instant;

#[tokio::test]
async fn test_onnx_nmt_adapter_mock_inference() {
    let mut src_tok = SentencePieceTokenizer::default();
    let mut tgt_tok = SentencePieceTokenizer::default();

    // Mock tokens
    src_tok.eos_id = 0;
    src_tok.pad_id = 1;
    tgt_tok.eos_id = 0;
    tgt_tok.pad_id = 1;

    let model = OnnxNmtModel::new_mock(src_tok, tgt_tok);
    let adapter = OnnxNmtAdapter::with_model(model);

    assert_eq!(adapter.engine_kind(), TranslationEngineKind::OnnxNmt);

    let start = Instant::now();
    let result = adapter
        .translate("Hello world, this is a live test.", "en", "zh")
        .await
        .expect("mock translation should succeed");
    let elapsed = start.elapsed();

    // Latency should be well under 20ms for lightweight inference
    assert!(
        elapsed.as_millis() < 20,
        "Translation took too long: {:?}",
        elapsed
    );
    assert_eq!(result, "");
}

#[tokio::test]
async fn test_onnx_nmt_adapter_unloaded_error() {
    let adapter = OnnxNmtAdapter::new(Some(Path::new("non_existent_model_dir").to_path_buf()), 2);
    let result = adapter.translate("Hello", "en", "zh").await;
    assert!(result.is_err());
}
