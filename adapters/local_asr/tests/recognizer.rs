use sona_core::model_config::ModelFileConfig;
use sona_local_asr::recognizer::{ModelType, build_model_config};
use std::path::Path;

#[test]
fn build_model_config_supports_qwen3_asr_without_tokens() {
    let model_path = Path::new("C:/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25");
    let file_config = Some(ModelFileConfig {
        conv_frontend: Some("conv_frontend.onnx".to_string()),
        encoder: Some("encoder.int8.onnx".to_string()),
        decoder: Some("decoder.int8.onnx".to_string()),
        tokenizer: Some("tokenizer".to_string()),
        ..Default::default()
    });

    let model = build_model_config(model_path, "qwen3-asr", &file_config, false, "auto", None)
        .expect("qwen3-asr model should build");

    match model {
        ModelType::OfflineQwen3Asr {
            conv_frontend,
            encoder,
            decoder,
            tokenizer,
            ..
        } => {
            assert_eq!(conv_frontend, model_path.join("conv_frontend.onnx"));
            assert_eq!(encoder, model_path.join("encoder.int8.onnx"));
            assert_eq!(decoder, model_path.join("decoder.int8.onnx"));
            assert_eq!(tokenizer, model_path.join("tokenizer"));
        }
        other => panic!("expected OfflineQwen3Asr, got {other:?}"),
    }
}

#[test]
fn build_model_config_supports_funasr_nano_without_tokens() {
    let model_path = Path::new("C:/models/funasr-nano");
    let file_config = Some(ModelFileConfig {
        encoder_adaptor: Some("encoder_adaptor.int8.onnx".to_string()),
        llm: Some("llm.int8.onnx".to_string()),
        embedding: Some("embedding.int8.onnx".to_string()),
        tokenizer: Some("Qwen3-0.6B".to_string()),
        ..Default::default()
    });

    let model = build_model_config(model_path, "funasr-nano", &file_config, false, "auto", None)
        .expect("funasr-nano should build without tokens");

    match model {
        ModelType::OfflineFunASRNano { tokens, .. } => {
            assert!(tokens.is_none());
        }
        other => panic!("expected OfflineFunASRNano, got {other:?}"),
    }
}

#[test]
fn build_model_config_still_requires_tokens_for_sensevoice() {
    let model_path = Path::new("C:/models/sensevoice");
    let file_config = Some(ModelFileConfig {
        model: Some("model.int8.onnx".to_string()),
        ..Default::default()
    });

    let error = build_model_config(model_path, "sensevoice", &file_config, true, "auto", None)
        .expect_err("sensevoice should still require tokens.txt");

    assert!(
        error.contains("Required file name not specified in config"),
        "unexpected error: {error}"
    );
}
