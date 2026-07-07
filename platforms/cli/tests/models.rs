use serde_json::Value;

#[test]
fn models_list_outputs_table_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("ID"));
    assert!(output.stdout.contains("Type"));
    assert!(output.stdout.contains("Language"));
    assert!(output.stdout.contains("Installed"));
    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(output.stdout.contains("silero-vad"));
    assert!(!output.stdout.contains("\"installed\""));
}

#[test]
fn models_list_outputs_json_with_json_flag() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "--json",
    ])
    .unwrap();
    let value: Value = serde_json::from_str(&output.stdout).unwrap();
    let models = value.as_array().unwrap();

    assert_eq!(output.stderr, "");
    assert!(models.iter().any(|model| {
        model["id"] == "sherpa-onnx-whisper-turbo"
            && model["type"] == "whisper"
            && model["installed"] == false
            && model["install_path"].is_string()
    }));
}

#[test]
fn models_list_can_filter_by_mode_type_and_language() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "--mode",
        "batch",
        "--type",
        "whisper",
        "--language",
        "zh",
    ])
    .unwrap();

    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(output.stdout.contains("sherpa-onnx-whisper-large-v3"));
    assert!(!output.stdout.contains("silero-vad"));
    assert!(
        !output
            .stdout
            .contains("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30")
    );
}

#[test]
fn models_list_can_filter_installed_only() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
        "--installed",
    ])
    .unwrap();

    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(!output.stdout.contains("silero-vad"));
}

#[test]
fn models_delete_yes_removes_installed_directory_model() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let install_path = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&install_path).unwrap();
    std::fs::write(install_path.join("model.onnx"), "fake").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "sherpa-onnx-whisper-turbo",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
        "--yes",
    ])
    .unwrap();

    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Deleted sherpa-onnx-whisper-turbo"));
    assert!(!install_path.exists());
}

#[test]
fn models_delete_yes_removes_installed_file_model() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let install_path = models_dir.join("silero_vad.onnx");
    std::fs::create_dir_all(&models_dir).unwrap();
    std::fs::write(&install_path, "fake").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "silero-vad",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
        "--yes",
    ])
    .unwrap();

    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Deleted silero-vad"));
    assert!(!install_path.exists());
}

#[test]
fn models_delete_missing_model_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "sherpa-onnx-whisper-turbo",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
        "--yes",
    ])
    .unwrap();

    assert_eq!(output.stdout, "");
    assert!(
        output
            .stderr
            .contains("Model sherpa-onnx-whisper-turbo is not installed")
    );
}

#[test]
fn models_delete_unknown_model_returns_usage_failure() {
    let dir = tempfile::tempdir().unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "not-a-real-model",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "--yes",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("Unknown model id"));
}
