#[test]
fn top_level_help_exposes_only_stateless_cli_commands() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "--help"])
        .expect("clap help should succeed with exit code 0");
    assert_eq!(output.stderr, "");
    let help = output.stdout;

    for command in [
        "diagnostics",
        "export",
        "init-config",
        "models",
        "path-status",
        "serve",
        "transcribe",
        "transcribe-live",
    ] {
        assert!(help.contains(command), "help must expose {command}");
    }
    for removed in [
        "app-config",
        "automation",
        "backup",
        "dashboard",
        "history",
        "llm",
        "recovery",
        "storage",
        "task-ledger",
    ] {
        assert!(!help.contains(removed), "help must not expose {removed}");
    }
}

#[test]
fn top_level_version_succeeds_and_outputs_to_stdout() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "--version"])
        .expect("clap version should succeed with exit code 0");
    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("sona-cli"));
}

#[test]
fn file_transcription_help_exposes_online_asr_options() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--help"])
        .expect("clap subcommand help should succeed with exit code 0");
    assert_eq!(output.stderr, "");
    let help = output.stdout;

    assert!(help.contains("--online-provider"));
    assert!(help.contains("--api-key-env"));
    assert!(help.contains("--online-config"));
    assert!(help.contains("groq-whisper"));
    assert!(help.contains("mistral-voxtral"));
    assert!(help.contains("volcengine-doubao"));
}

#[test]
fn transcribe_and_serve_help_expose_ffmpeg_path() {
    let transcribe_help = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--help"])
        .unwrap()
        .stdout;
    assert!(transcribe_help.contains("--ffmpeg-path"));

    let serve_help = sona_cli::run_cli_from_args(["sona-cli", "serve", "--help"])
        .unwrap()
        .stdout;
    assert!(serve_help.contains("--ffmpeg-path"));
}

#[test]
fn online_batch_requires_api_key_from_the_named_environment_variable() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("audio.wav");
    std::fs::write(&input, b"not opened before credential validation").unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        input.to_str().unwrap(),
        "--online-provider",
        "groq-whisper",
        "--api-key-env",
        "SONA_CLI_TEST_MISSING_ASR_KEY_8D6D7E15",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("SONA_CLI_TEST_MISSING_ASR_KEY_8D6D7E15")
    );
}

#[test]
fn online_live_rejects_batch_only_providers_before_opening_audio_input() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--online-provider",
        "groq-whisper",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("does not support streaming"));
}

#[test]
fn online_asr_rejects_local_model_options() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "missing.wav",
        "--online-provider",
        "groq-whisper",
        "--model-id",
        "local-model",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "--model-id can only be used with local ASR."
    );
}

#[test]
fn local_transcribe_requires_existing_input_file() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "transcribe", "nonexistent-audio-file.wav"])
            .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "Input file must be an existing file: nonexistent-audio-file.wav"
    );
}
