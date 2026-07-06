use std::path::PathBuf;

use sona_core::transcribe_runtime::OfflineTranscribeOptions;

#[test]
fn offline_transcribe_options_are_not_cli_specific() {
    let options = OfflineTranscribeOptions {
        input: PathBuf::from("sample.wav"),
        output: None,
        format: None,
        language: None,
        model_id: None,
        models_dir: None,
        vad_model_id: None,
        punctuation_model_id: None,
        threads: None,
        enable_itn: None,
        hotwords: None,
        gpu_acceleration: None,
        vad_buffer: None,
        save_wav: None,
        quiet: false,
        force: false,
    };

    assert_eq!(options.input, PathBuf::from("sample.wav"));
    assert!(!options.quiet);
    assert!(!options.force);
}
