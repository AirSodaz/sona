use sona_local_asr::audio::load_vad;

#[test]
fn load_vad_ignores_missing_configuration() {
    let vad = load_vad(None);

    assert!(vad.is_none());
}

#[test]
fn load_vad_ignores_empty_model_path() {
    let vad = load_vad(Some(String::new()));

    assert!(vad.is_none());
}
