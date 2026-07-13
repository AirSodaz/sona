#[test]
fn transcribe_live_command_exposes_the_public_input_and_output_flags() {
    let error = sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--help"])
        .expect_err("clap help is returned through the usage error path");
    let help = error.to_string();

    assert!(help.contains("Transcribe live audio"));
    assert!(help.contains("--input"));
    assert!(help.contains("--device"));
    assert!(help.contains("--list-input-devices"));
    assert!(help.contains("--duration"));
    assert!(help.contains("--output-format"));
    assert!(help.contains("--output"));
    assert!(help.contains("--force"));
}

#[test]
fn transcribe_live_rejects_zero_duration_before_model_resolution() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--duration",
        "0",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(error.to_string(), "--duration must be greater than 0.");
}

#[test]
fn transcribe_live_rejects_device_for_stdin_before_model_resolution() {
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe-live",
        "--input",
        "stdin",
        "--device",
        "Studio Mic",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "--device can only be used with microphone input."
    );
}

#[test]
fn transcribe_live_requires_a_streaming_model_before_opening_input() {
    let error = sona_cli::run_cli_from_args(["sona-cli", "transcribe-live", "--input", "stdin"])
        .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "Missing required streaming model. Pass --model-id or set model_id in --config."
    );
}
