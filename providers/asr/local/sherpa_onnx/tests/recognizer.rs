use sona_core::models::config::ModelFileConfig;
use sona_core::ports::asr::AsrPortErrorKind;
use sona_sherpa_onnx::recognizer::{
    ModelType, OfflineDecodeResult, build_model_config, build_offline_model_config,
    create_offline_recognizer,
};
use std::path::Path;

#[test]
fn build_model_config_supports_qwen3_asr_without_tokens() {
    let model_path = Path::new("C:/models/qwen3-asr-0.6b-q8-gguf");
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
fn build_model_config_supports_parakeet_tdt() {
    let model_path = Path::new("C:/models/parakeet-tdt");
    let file_config = Some(ModelFileConfig {
        encoder: Some("encoder.int8.onnx".to_string()),
        decoder: Some("decoder.int8.onnx".to_string()),
        joiner: Some("joiner.int8.onnx".to_string()),
        tokens: Some("tokens.txt".to_string()),
        ..Default::default()
    });

    let model = build_model_config(
        model_path,
        "parakeet-tdt",
        &file_config,
        false,
        "auto",
        None,
    )
    .expect("parakeet-tdt model should build");

    match model {
        ModelType::OfflineParakeetTdt {
            encoder,
            decoder,
            joiner,
            tokens,
        } => {
            assert_eq!(encoder, model_path.join("encoder.int8.onnx"));
            assert_eq!(decoder, model_path.join("decoder.int8.onnx"));
            assert_eq!(joiner, model_path.join("joiner.int8.onnx"));
            assert_eq!(tokens, model_path.join("tokens.txt"));
        }
        other => panic!("expected OfflineParakeetTdt, got {other:?}"),
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
        error
            .message
            .contains("Required file name not specified in config"),
        "unexpected error: {error}"
    );
    assert_eq!(error.kind, AsrPortErrorKind::Model);
}

#[test]
fn build_offline_model_config_rejects_online_model_type_before_file_validation() {
    let model_path = Path::new("C:/models/zipformer");
    let file_config = Some(ModelFileConfig::default());

    let error =
        build_offline_model_config(model_path, "zipformer", &file_config, false, "auto", None)
            .expect_err("offline transcription should reject online model types");

    assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(error.message, "Unsupported offline model type: zipformer");
}

#[test]
fn create_offline_recognizer_rejects_online_model_type() {
    let model_type = ModelType::OnlineParaformer {
        encoder: "encoder.onnx".into(),
        decoder: "decoder.onnx".into(),
        tokens: "tokens.txt".into(),
    };

    let error = match create_offline_recognizer(model_type, 1, Some("cpu")) {
        Err(error) => error,
        Ok(_) => panic!("online recognizer variants cannot be used for offline transcription"),
    };

    assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    assert_eq!(error.message, "Unsupported offline model type: paraformer");
}

#[test]
fn offline_decode_result_reports_empty_text() {
    let result = OfflineDecodeResult {
        text: "   ".to_string(),
        tokens: Vec::new(),
        timestamps: None,
    };

    assert!(result.is_empty_text());
}
