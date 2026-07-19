use sona_core::ports::asr::AsrPortErrorKind;
use sona_local_asr::punctuation::load_punctuation_from_path;

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
