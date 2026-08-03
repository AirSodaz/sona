#[test]
fn top_level_help_exposes_only_stateless_cli_commands() {
    let help = sona_cli::run_cli_from_args(["sona-cli", "--help"])
        .expect_err("clap help is returned through the usage error path")
        .to_string();

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
fn file_transcription_help_exposes_online_asr_options() {
    let help = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--help"])
        .expect_err("clap help is returned through the usage error path")
        .to_string();

    assert!(help.contains("--online-provider"));
    assert!(help.contains("--api-key-env"));
    assert!(help.contains("--online-config"));
    assert!(help.contains("groq-whisper"));
    assert!(help.contains("mistral-voxtral"));
    assert!(help.contains("volcengine-doubao"));
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
