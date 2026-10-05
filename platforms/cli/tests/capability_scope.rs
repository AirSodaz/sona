#[test]
fn top_level_help_exposes_only_stateless_cli_commands() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "--help"])
        .expect("clap help should succeed with exit code 0");
    assert_eq!(output.stderr, "");
    let help = output.stdout;

    for command in [
        "completion",
        "config",
        "devices",
        "doctor",
        "export",
        "models",
        "providers",
        "serve",
        "transcribe",
        "transcribe-live",
    ] {
        assert!(help.contains(command), "help must expose {command}");
    }
    for heading in [
        "Transcription:",
        "Resources & Devices:",
        "Conversion & Formatting:",
        "Configuration & Service:",
        "System & Diagnostics:",
    ] {
        assert!(help.contains(heading), "help must expose heading {heading}");
    }
    assert!(
        !help.contains("\n\n\n"),
        "help output must not have excessive blank lines"
    );
    for hidden_or_removed in [
        "app-config",
        "automation",
        "backup",
        "dashboard",
        "diagnostics",
        "history",
        "init-config",
        "llm",
        "path-status",
        "recovery",
        "storage",
        "task-ledger",
    ] {
        assert!(
            !help.contains(hidden_or_removed),
            "help must not expose {hidden_or_removed}"
        );
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
fn completion_generates_valid_scripts_for_supported_shells() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let output = sona_cli::run_cli_from_args(["sona-cli", "completion", shell])
            .expect("completion command should succeed");
        assert_eq!(output.stderr, "");
        assert!(
            !output.stdout.is_empty(),
            "shell script for {shell} must not be empty"
        );
        assert!(
            output.stdout.contains("sona-cli"),
            "shell script must contain command name"
        );
    }
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
        "--models-dir",
        "./models",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "--models-dir can only be used with local ASR."
    );
}

#[test]
fn online_asr_rejects_conflicting_model_arguments() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "missing.wav",
        "--online-provider",
        "groq-whisper",
        "-m",
        "whisper-large-v3",
        "--online-model",
        "distil-whisper",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("Conflicting model names"));
}

#[test]
fn online_asr_accepts_unified_model_flag() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("audio.wav");
    std::fs::write(&input, b"dummy audio content").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        input.to_str().unwrap(),
        "--online-provider",
        "groq-whisper",
        "-m",
        "whisper-large-v3",
        "--api-key-env",
        "SONA_CLI_TEST_MISSING_KEY_XYZ",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    // It validates API key presence rather than rejecting -m, proving -m was accepted as online model
    assert!(error.to_string().contains("SONA_CLI_TEST_MISSING_KEY_XYZ"));
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

#[test]
fn transcribe_with_config_provider_and_cli_api_key_override_resolves_online() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("audio.wav");
    std::fs::write(&input, b"dummy audio content").unwrap();

    let config_path = directory.path().join("sona-cli.toml");
    std::fs::write(
        &config_path,
        r#"
[transcribe]
online_provider = "groq-whisper"
"#,
    )
    .unwrap();

    // Passing a whitespace --api-key proves that:
    // 1. Clap parses --api-key without requiring --online-provider on CLI
    // 2. config online_provider is resolved
    // 3. business validation correctly checks the CLI --api-key override
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        input.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--api-key",
        "   ",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(error.to_string(), "--api-key must not be empty.");
}

#[test]
fn transcribe_with_config_provider_and_cli_model_resolves_online() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("audio.wav");
    std::fs::write(&input, b"dummy audio content").unwrap();

    let config_path = directory.path().join("sona-cli.toml");
    std::fs::write(
        &config_path,
        r#"
[transcribe]
online_provider = "groq-whisper"
api_key_env = "SONA_CLI_TEST_CONFIG_PROVIDER_KEY_8F1A"
"#,
    )
    .unwrap();

    // If CLI -m dropped config provider, this would fail looking for local model.
    // With unified -m and config merging, it resolves groq-whisper with model whisper-large-v3,
    // and fails at missing api_key_env, proving online resolution succeeded.
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        input.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "-m",
        "whisper-large-v3",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("SONA_CLI_TEST_CONFIG_PROVIDER_KEY_8F1A")
    );
}

#[test]
fn live_with_config_provider_and_cli_model_resolves_online() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("sona-cli.toml");
    std::fs::write(
        &config_path,
        r#"
[transcribe_live]
online_provider = "volcengine-doubao"
api_key_env = "SONA_CLI_TEST_LIVE_CONFIG_PROVIDER_KEY_8F1A"
"#,
    )
    .unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "live",
        "--input",
        "stdin",
        "--config",
        config_path.to_str().unwrap(),
        "-m",
        "doubao-streaming",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("SONA_CLI_TEST_LIVE_CONFIG_PROVIDER_KEY_8F1A")
    );
}

#[test]
fn transcribe_rejects_cli_api_key_when_no_provider_configured() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("audio.wav");
    std::fs::write(&input, b"dummy audio content").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        input.to_str().unwrap(),
        "--api-key",
        "gsk_orphan_key",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("--api-key requires an online ASR provider")
    );
}

#[test]
fn transcribe_live_rejects_cli_api_key_when_no_provider_configured() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--api-key",
        "orphan_key",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("--api-key requires an online ASR provider")
    );
}

#[test]
fn transcribe_live_with_config_provider_and_cli_api_key_resolves_online() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("sona-cli.toml");
    std::fs::write(
        &config_path,
        r#"
[transcribe_live]
online_provider = "groq-whisper"
"#,
    )
    .unwrap();

    // groq-whisper does not support streaming transcription.
    // If Clap failed on --api-key requiring --online-provider, it would error on Clap parsing.
    // Here it parses, resolves groq-whisper from config, applies --api-key, and fails with
    // business validation: "groq-whisper does not support streaming transcription."
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--config",
        config_path.to_str().unwrap(),
        "--api-key",
        "test_secret_key",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Online ASR provider groq-whisper does not support streaming transcription.")
    );
}

#[test]
fn transcribe_batch_rejects_missing_input_file() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "missing_file_1.wav",
        "missing_file_2.wav",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Input file must be an existing file")
    );
}

#[test]
fn transcribe_batch_rejects_nonexistent_input_dir() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "--input-dir",
        "nonexistent_recordings_dir_xyz",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("--input-dir must be an existing directory")
    );
}

#[test]
fn transcribe_batch_multiple_files_resolves_and_validates_batch_pipeline() {
    let directory = tempfile::tempdir().unwrap();
    let file1 = directory.path().join("meeting1.wav");
    let file2 = directory.path().join("meeting2.wav");
    std::fs::write(&file1, b"audio 1 content").unwrap();
    std::fs::write(&file2, b"audio 2 content").unwrap();

    let out_dir = directory.path().join("transcripts");
    std::fs::create_dir(&out_dir).unwrap();

    // Tests multi-input path batch with whitespace api-key, verifying batch input planning runs
    // and reaches credential validation deterministically without network calls
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        "--output-dir",
        out_dir.to_str().unwrap(),
        "--online-provider",
        "groq-whisper",
        "--api-key",
        "   ",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(error.to_string(), "--api-key must not be empty.");
}

#[test]
fn transcribe_batch_rejects_output_file_flag() {
    let directory = tempfile::tempdir().unwrap();
    let file1 = directory.path().join("audio1.wav");
    let file2 = directory.path().join("audio2.wav");
    std::fs::write(&file1, b"audio 1 content").unwrap();
    std::fs::write(&file2, b"audio 2 content").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        "-o",
        "out.srt",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("--output cannot be used with multiple input files")
    );
}

#[test]
fn transcribe_rejects_conflicting_output_and_output_dir() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "audio.wav",
        "-o",
        "out.srt",
        "--output-dir",
        "out_dir",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("cannot be used with"));
}

#[test]
fn transcribe_list_providers_outputs_table_with_all_providers() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--list-providers"])
        .expect("list-providers should succeed");

    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("PROVIDER"));
    assert!(output.stdout.contains("DEFAULT_ENV_VAR"));
    assert!(output.stdout.contains("MODES"));
    assert!(output.stdout.contains("groq-whisper"));
    assert!(output.stdout.contains("volcengine-doubao"));
    assert!(output.stdout.contains("openai-whisper"));
    // Regression test: volcengine-doubao must support both batch and streaming in manifest
    let volcengine_line = output
        .stdout
        .lines()
        .find(|line| line.contains("volcengine-doubao"))
        .expect("volcengine-doubao must be present in providers table");
    assert!(
        volcengine_line.contains("batch, streaming"),
        "volcengine-doubao must support both batch and streaming, got: {volcengine_line}"
    );
    let groq_line = output
        .stdout
        .lines()
        .find(|line| line.contains("groq-whisper"))
        .expect("groq-whisper must be present in providers table");
    assert!(
        !groq_line.contains("streaming"),
        "groq-whisper must be batch-only, got: {groq_line}"
    );
}
#[test]
fn transcribe_live_list_providers_matches_transcribe_output() {
    let batch_output = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--list-providers"])
        .expect("batch list-providers should succeed");
    let live_output =
        sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--list-providers"])
            .expect("live list-providers should succeed");

    assert_eq!(batch_output.stderr, "");
    assert_eq!(live_output.stderr, "");
    assert_eq!(batch_output.stdout, live_output.stdout);
}

#[test]
fn transcribe_live_rejects_conflicting_list_flags() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--list-input-devices",
        "--list-providers",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("cannot be used with"));
}

#[test]
fn transcribe_jobs_zero_rejected() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "transcribe", "sample.wav", "--jobs", "0"])
            .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("--jobs must be greater than 0"));
}

#[test]
fn transcribe_jobs_on_single_file_reports_validation_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("audio.wav");
    std::fs::write(&file, b"dummy").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        file.to_str().unwrap(),
        "-m",
        "whisper-turbo",
        "--jobs",
        "2",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("--jobs can only be used in batch transcription mode")
    );
}

#[test]
fn transcribe_jobs_greater_than_one_reports_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("audio1.wav");
    let file2 = dir.path().join("audio2.wav");
    std::fs::write(&file1, b"dummy1").unwrap();
    std::fs::write(&file2, b"dummy2").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        "-m",
        "whisper-turbo",
        "--jobs",
        "2",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Concurrent batch transcription (--jobs > 1) is experimental; current execution is sequential")
    );
}

#[test]
fn transcribe_rejects_invalid_gpu_acceleration() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "sample.wav",
        "--gpu-acceleration",
        "invalid_accel",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn serve_rejects_invalid_gpu_acceleration() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "serve", "--gpu-acceleration", "invalid_accel"])
            .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn doctor_outputs_readable_status_summary() {
    let output =
        sona_cli::run_cli_from_args(["sona-cli", "doctor"]).expect("doctor command should succeed");

    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("Sona CLI System Health Check:"));
    assert!(output.stdout.contains("FFmpeg:"));
    assert!(output.stdout.contains("Audio Input:"));
    assert!(output.stdout.contains("Models:"));
    assert!(output.stdout.contains("Hardware Acceleration:"));
    assert!(output.stdout.contains("Configuration:"));
}

#[test]
fn doctor_json_outputs_valid_schema() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "doctor", "--json"])
        .expect("doctor --json command should succeed");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert!(json["all_ok"].is_boolean());
    assert!(json["ffmpeg"]["found"].is_boolean());
    assert!(json["audio_input"]["available"].is_boolean());
    assert!(json["models"]["path"].is_string());
    assert!(json["hardware_acceleration"]["available_modes"].is_array());
    assert!(json["config"]["valid"].is_boolean());
}

#[test]
fn doctor_with_missing_ffmpeg_reports_warning_without_failing() {
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "doctor",
        "--ffmpeg-path",
        "/path/that/definitely/does/not/exist/ffmpeg",
        "--json",
    ])
    .expect("doctor should succeed even with missing ffmpeg");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(json["ffmpeg"]["found"], false);
    assert_eq!(json["all_ok"], false);
}

#[test]
fn doctor_with_invalid_config_reports_failure_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let bad_config = dir.path().join("invalid.toml");
    std::fs::write(&bad_config, "this is not valid toml = [[[").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "doctor",
        "--config",
        bad_config.to_string_lossy().as_ref(),
        "--json",
    ])
    .expect("doctor should succeed even with invalid config to report status");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(json["config"]["found"], true);
    assert_eq!(json["config"]["valid"], false);
    assert_eq!(json["all_ok"], false);
}

#[test]
fn doctor_strict_mode_fails_with_exit_code_1_on_warnings() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "doctor",
        "--ffmpeg-path",
        "/path/that/definitely/does/not/exist/ffmpeg",
        "--strict",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 1);
    assert!(error.to_string().contains("Doctor health check failed"));
}

#[test]
fn devices_command_outputs_device_list_or_empty() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "devices"])
        .expect("devices command should succeed");

    assert_eq!(output.stderr, "");
}

#[test]
fn devices_command_json_outputs_valid_schema() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "devices", "-j"])
        .expect("devices -j command should succeed");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert!(json["devices"].is_array());
}

#[test]
fn providers_command_matches_transcribe_list_providers() {
    let providers_output = sona_cli::run_cli_from_args(["sona-cli", "providers"])
        .expect("providers command should succeed");
    let list_output = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--list-providers"])
        .expect("transcribe --list-providers should succeed");

    assert_eq!(providers_output.stderr, "");
    assert_eq!(providers_output.stdout, list_output.stdout);
}

#[test]
fn providers_command_json_outputs_valid_schema() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "providers", "--json"])
        .expect("providers --json command should succeed");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    assert!(arr.iter().any(|p| p["id"] == "volcengine-doubao"));
    assert!(arr.iter().any(|p| p["id"] == "groq-whisper"));
    assert!(arr.iter().all(|p| p.get("configured").is_some()));
}
#[test]
fn providers_command_with_models_flag_lists_curated_models() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "providers", "--models"])
        .expect("providers --models should succeed");

    assert_eq!(output.stderr, "");
    assert!(
        output
            .stdout
            .contains("Supported Online ASR Providers & Models:")
    );
    assert!(output.stdout.contains("volcengine-doubao"));
    assert!(output.stdout.contains("volc.bigasr.auc_turbo"));
    assert!(output.stdout.contains("groq-whisper"));
    assert!(output.stdout.contains("whisper-large-v3"));
}

#[test]
fn providers_command_inspect_single_provider_outputs_details_and_models() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "providers", "groq-whisper"])
        .expect("providers groq-whisper should succeed");

    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("Provider: groq-whisper"));
    assert!(output.stdout.contains("Env: GROQ_API_KEY"));
    assert!(output.stdout.contains("whisper-large-v3"));
    assert!(output.stdout.contains("Usage Examples:"));
    assert!(output.stdout.contains("--online-provider groq-whisper"));
}

#[test]
fn providers_command_inspect_single_provider_json() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "providers", "groq-whisper", "--json"])
        .expect("providers groq-whisper --json should succeed");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(json["id"], "groq-whisper");
    assert_eq!(json["default_env_var"], "GROQ_API_KEY");
    assert!(json["models"].is_array());
    assert!(
        json["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "whisper-large-v3")
    );
}

#[test]
fn providers_command_unknown_provider_returns_validation_error() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "providers", "nonexistent-provider"]).unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Unknown online ASR provider 'nonexistent-provider'")
    );
}

#[test]
fn providers_command_json_includes_models_array() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "providers", "--json"])
        .expect("providers --json command should succeed");

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    let arr = json.as_array().unwrap();
    let groq = arr.iter().find(|p| p["id"] == "groq-whisper").unwrap();
    assert!(groq["models"].is_array());
    assert!(
        groq["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "whisper-large-v3")
    );
}

#[test]
fn config_init_accepts_short_force_flag() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(&config_path, "existing").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "init",
        config_path.to_string_lossy().as_ref(),
        "-F",
    ])
    .expect("config init with -F should succeed");

    assert!(output.stderr.contains("Created config template"));
}

#[test]
fn transcribe_stdin_rejects_terminal_input() {
    let error = sona_cli::run_cli_from_args_with_terminal_stdin([
        "sona-cli",
        "transcribe",
        "-",
        "-m",
        "whisper-turbo",
    ])
    .unwrap_err();
    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("No audio input provided via stdin")
    );
}

#[test]
fn transcribe_stdin_rejects_empty_input() {
    let error = sona_cli::run_cli_from_args_with_stdin(
        ["sona-cli", "transcribe", "-", "-m", "whisper-turbo"],
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("Standard input was empty"));
}

#[test]
fn transcribe_mode_flag_validation() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        "audio.wav",
        "--mode",
        "invalid-mode",
    ])
    .unwrap_err();
    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("invalid-mode"));
}

#[test]
fn transcribe_stream_format_alias_in_transcribe_live() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--help"]).unwrap();
    assert!(output.stdout.contains("--stream-format"));
}

#[test]
fn transcribe_stdin_with_audio_bytes_buffers_and_proceeds_to_model_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args_with_stdin(
        [
            "sona-cli",
            "transcribe",
            "-",
            "-m",
            "whisper-turbo",
            "--models-dir",
            dir.path().to_string_lossy().as_ref(),
        ],
        b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x80>\x00\x00\x00}\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00".to_vec(),
    )
    .unwrap_err();
    // Proves stdin buffering finished and execution proceeded into model path resolution
    assert_eq!(error.exit_code(), 2);
    assert!(
        error.to_string().contains("sherpa-onnx-whisper-turbo")
            && error.to_string().contains("was not found"),
        "Expected model resolution error for stdin input, got: {error}"
    );
    assert!(!error.to_string().contains("stdin"));
    assert!(!error.to_string().contains("Standard input was empty"));
    assert!(
        !error
            .to_string()
            .contains("No audio input provided via stdin")
    );
}

#[test]
fn transcribe_auto_infers_single_installed_batch_model() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let whisper_dir = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&whisper_dir).unwrap();
    std::fs::write(whisper_dir.join("turbo-encoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(whisper_dir.join("turbo-decoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(whisper_dir.join("turbo-tokens.txt"), b"fake").unwrap();

    let fake_audio = dir.path().join("sample.wav");
    std::fs::write(&fake_audio, b"fake audio").unwrap();

    // Without -m/--model-id, transcribe should automatically select the only installed batch model (whisper-turbo)
    // rather than failing with "Missing required batch model"
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        fake_audio.to_string_lossy().as_ref(),
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(
        !error.to_string().contains("Missing required batch model"),
        "Expected auto-inference of single installed model, got: {error}"
    );
}

#[test]
fn transcribe_requires_model_id_when_multiple_batch_models_installed() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    // 1. Install whisper-turbo
    let whisper_dir = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&whisper_dir).unwrap();
    std::fs::write(whisper_dir.join("turbo-encoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(whisper_dir.join("turbo-decoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(whisper_dir.join("turbo-tokens.txt"), b"fake").unwrap();

    // 2. Install sensevoice
    let sensevoice_dir = models_dir.join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17");
    std::fs::create_dir_all(&sensevoice_dir).unwrap();
    std::fs::write(sensevoice_dir.join("model.int8.onnx"), b"fake").unwrap();
    std::fs::write(sensevoice_dir.join("tokens.txt"), b"fake").unwrap();

    let fake_audio = dir.path().join("sample.wav");
    std::fs::write(&fake_audio, b"fake audio").unwrap();

    // Multiple installed batch models -> cannot auto-infer, must fail with "Missing required batch model"
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        fake_audio.to_string_lossy().as_ref(),
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(
        error.to_string().contains("Missing required batch model"),
        "Expected error requiring model_id with multiple candidates, got: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("sona-cli models download whisper-turbo"),
        "Expected actionable hint in missing model error, got: {error}"
    );
}
#[test]
fn direct_init_config_subcommand_fails_as_unrecognized_command() {
    let error = sona_cli::run_cli_from_args(["sona-cli", "init-config"]).unwrap_err();
    assert_eq!(error.exit_code(), 2);
    let msg = error.to_string();
    assert!(
        msg.contains("unrecognized subcommand 'init-config'")
            || msg.contains("error: unrecognized subcommand"),
        "Expected unrecognized subcommand error, got: {msg}"
    );
}

#[test]
fn transcribe_auto_detects_piped_stdin_when_dash_is_omitted() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args_with_stdin(
        [
            "sona-cli",
            "transcribe",
            "-m",
            "whisper-turbo",
            "--models-dir",
            dir.path().to_string_lossy().as_ref(),
        ],
        b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x80>\x00\x00\x00}\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00".to_vec(),
    )
    .unwrap_err();
    assert_eq!(error.exit_code(), 2);
    assert!(
        error.to_string().contains("sherpa-onnx-whisper-turbo")
            && error.to_string().contains("was not found"),
        "Expected model resolution error, got: {error}"
    );
    assert!(!error.to_string().contains("Missing input"));
}

#[test]
fn transcribe_terminal_stdin_without_inputs_reports_missing_input() {
    let error =
        sona_cli::run_cli_from_args_with_terminal_stdin(["sona-cli", "transcribe"]).unwrap_err();
    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("Missing input"));
}

#[test]
fn transcribe_accepts_provider_alias_for_online_provider() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("audio.wav");
    std::fs::write(&file, b"dummy audio content").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        file.to_str().unwrap(),
        "--provider",
        "groq-whisper",
    ])
    .unwrap_err();
    assert!(
        error.to_string().contains("API key") || error.to_string().contains("GROQ_API_KEY"),
        "Expected provider-specific API key error proving --provider was parsed, got: {error}"
    );
}
