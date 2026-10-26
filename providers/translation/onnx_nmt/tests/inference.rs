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
    src_tok.pad_id = Some(1);
    tgt_tok.eos_id = 0;
    tgt_tok.pad_id = Some(1);

    let model = OnnxNmtModel::new_mock(src_tok, tgt_tok);
    let adapter = OnnxNmtAdapter::with_model(model);

    assert_eq!(adapter.engine_kind(), TranslationEngineKind::OnnxNmt);

    let start = Instant::now();
    let result = adapter
        .translate("Hello world, this is a live test.", "en", "zh")
        .await
        .expect("mock translation should succeed");
    let elapsed = start.elapsed();

    // Generous hang-prevention check for test execution
    assert!(
        elapsed.as_millis() < 2000,
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

#[test]
fn test_leading_prompt_token_slice_decoding() {
    use sona_translation_onnx::PieceEntry;

    let mut tgt_tok = SentencePieceTokenizer::default();
    tgt_tok.pieces = vec![
        PieceEntry {
            piece: "</s>".to_string(),
            score: 0.0,
            piece_type: 0,
        },
        PieceEntry {
            piece: "<pad>".to_string(),
            score: 0.0,
            piece_type: 0,
        },
        PieceEntry {
            piece: "\u{2581}你好".to_string(),
            score: -1.0,
            piece_type: 0,
        },
        PieceEntry {
            piece: "\u{2581}世界".to_string(),
            score: -1.0,
            piece_type: 0,
        },
    ];
    tgt_tok.eos_id = 0;
    tgt_tok.pad_id = Some(1);

    // Simulated raw decoder_input_ids generated during auto-regression:
    // Index 0 is the start prompt token (pad_id = 1), followed by actual generated tokens [2, 3]
    let raw_decoder_input_ids = vec![1i64, 2i64, 3i64];

    // If not sliced, decoding stops immediately at index 0 because 1 == pad_id
    let unstripped = tgt_tok.decode(&raw_decoder_input_ids);
    assert_eq!(unstripped, "");

    // When sliced &[1..], it decodes the actual translated tokens
    let output_tokens = if raw_decoder_input_ids.len() > 1 {
        &raw_decoder_input_ids[1..]
    } else {
        &[]
    };
    let stripped = tgt_tok.decode(output_tokens);
    assert_eq!(stripped, "你好世界");
}

#[test]
fn test_sentencepiece_cjk_aware_decoding() {
    use sona_translation_onnx::tokenizer::decode_sentencepiece_text;

    // Pure CJK without spaces
    assert_eq!(
        decode_sentencepiece_text("\u{2581}你好\u{2581}世界"),
        "你好世界"
    );
    assert_eq!(
        decode_sentencepiece_text("\u{2581}你好\u{2581}，\u{2581}世界\u{2581}！"),
        "你好，世界！"
    );

    // Pure Latin with word boundary spaces preserved
    assert_eq!(
        decode_sentencepiece_text("\u{2581}Hello\u{2581}world"),
        "Hello world"
    );
    assert_eq!(
        decode_sentencepiece_text("\u{2581}Hello\u{2581}world\u{2581}!"),
        "Hello world !"
    );

    // Mixed CJK and Latin
    assert_eq!(
        decode_sentencepiece_text("\u{2581}你好\u{2581}Sona"),
        "你好 Sona"
    );
    assert_eq!(
        decode_sentencepiece_text("\u{2581}Sona\u{2581}你好"),
        "Sona 你好"
    );
    assert_eq!(
        decode_sentencepiece_text("\u{2581}模型\u{2581}v2\u{2581}发布"),
        "模型 v2 发布"
    );

    // Edge cases: empty, single chars, only markers
    assert_eq!(decode_sentencepiece_text(""), "");
    assert_eq!(decode_sentencepiece_text("\u{2581}\u{2581}\u{2581}"), "");
    assert_eq!(decode_sentencepiece_text("\u{2581}中\u{2581}"), "中");
}
#[tokio::test]
async fn test_onnx_nmt_real_opus_mt_en_zh() {
    let model_path = Path::new("D:/projects/models/opus-mt-en-zh");
    if !model_path.join("encoder_model.onnx").exists() || !model_path.join("vocab.json").exists() {
        return;
    }
    let adapter = OnnxNmtAdapter::new(Some(model_path.to_path_buf()), 2);
    let r1 = adapter
        .translate("Good morning.", "en", "zh")
        .await
        .expect("translation should succeed");
    println!("Translated 'Good morning.' -> {:?}", r1);
    let r2 = adapter
        .translate("Thank you very much.", "en", "zh")
        .await
        .expect("translation should succeed");
    println!("Translated 'Thank you very much.' -> {:?}", r2);

    let r3 = adapter
        .translate("What is your name?", "en", "zh")
        .await
        .expect("translation should succeed");
    println!("Translated 'What is your name?' -> {:?}", r3);

    assert_eq!(r1, "早上好");
    assert!(!r1.contains("和呢报表"));
    assert!(!r1.contains("我刚从你家回来"));
    assert_eq!(r2, "非常感谢");
    assert!(r3.contains("叫什么") || r3.contains("名字"));
}
