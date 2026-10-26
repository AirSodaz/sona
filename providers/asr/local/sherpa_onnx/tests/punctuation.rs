use sona_core::ports::asr::AsrPortErrorKind;
use sona_sherpa_onnx::punctuation::load_punctuation_from_path;

#[test]
fn load_punctuation_from_path_ignores_missing_configuration() {
    let punctuation = load_punctuation_from_path(None).unwrap();

    assert!(punctuation.is_none());
}

#[test]
fn load_punctuation_from_path_rejects_missing_model_path() {
    let missing = std::env::temp_dir().join(format!("sona-punct-{}", uuid::Uuid::new_v4()));

    let error = match load_punctuation_from_path(Some(&missing)) {
        Err(error) => error,
        Ok(_) => panic!("missing punctuation model path should be rejected"),
    };

    assert_eq!(error.kind, AsrPortErrorKind::Model);
    assert!(error.message.contains("Model path does not exist"));
}

#[test]
fn load_punctuation_from_path_distinguishes_online_and_offline() {
    use std::fs;

    let root = std::env::temp_dir().join(format!("sona-punct-dispatch-{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let onnx_path = root.join("model.onnx");
    fs::write(&onnx_path, b"fake onnx").unwrap();

    // Without bpe.vocab: Offline path chosen -> fails to create OfflinePunctuation
    let err = match load_punctuation_from_path(Some(&root)) {
        Err(e) => e,
        Ok(_) => panic!("expected failure"),
    };
    assert_eq!(err.kind, AsrPortErrorKind::Model);
    assert!(err.message.contains("Failed to create OfflinePunctuation"));

    // With bpe.vocab: Online path chosen -> fails to create OnlinePunctuation
    let vocab_path = root.join("bpe.vocab");
    fs::write(&vocab_path, b"fake vocab").unwrap();
    let err = match load_punctuation_from_path(Some(&root)) {
        Err(e) => e,
        Ok(_) => panic!("expected failure"),
    };
    assert!(err.message.contains("Failed to create OnlinePunctuation"));

    let _ = fs::remove_dir_all(&root);
}
