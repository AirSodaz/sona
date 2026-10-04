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
            && model["aliases"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x == "whisper-turbo"))
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
    let install_path = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&install_path).unwrap();
    std::fs::write(install_path.join("turbo-encoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(install_path.join("turbo-decoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(install_path.join("turbo-tokens.txt"), b"fake").unwrap();

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

#[test]
fn models_delete_without_yes_in_non_interactive_shell_fails_with_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let install_path = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&install_path).unwrap();
    std::fs::write(install_path.join("model.onnx"), "fake").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "sherpa-onnx-whisper-turbo",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Cannot prompt for confirmation in non-interactive shell")
    );
    assert!(error.to_string().contains("--yes"));
}

#[test]
fn models_delete_without_yes_when_not_installed_reports_not_installed() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "sherpa-onnx-whisper-turbo",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
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
fn models_list_mode_live_filters_live_models() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "--mode",
        "live",
        "--type",
        "whisper",
    ])
    .unwrap();

    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(output.stdout.contains("sherpa-onnx-whisper-large-v3"));
}

#[test]
fn models_list_rejects_outdated_mode_offline_or_streaming() {
    let dir = tempfile::tempdir().unwrap();
    let error_offline = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "--mode",
        "offline",
    ])
    .unwrap_err();
    assert_eq!(error_offline.exit_code(), 2);

    let error_streaming = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "--mode",
        "streaming",
    ])
    .unwrap_err();
    assert_eq!(error_streaming.exit_code(), 2);
}

#[test]
fn models_delete_accepts_model_alias_and_short_flag() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let install_path = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&install_path).unwrap();
    std::fs::write(install_path.join("model.onnx"), "fake").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "whisper-turbo",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
        "-y",
    ])
    .unwrap();

    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Deleted"));
    assert!(!install_path.exists());
}

#[test]
fn models_list_accepts_short_flags() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "-m",
        "batch",
        "-t",
        "whisper",
    ])
    .unwrap();

    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(output.stdout.contains("sherpa-onnx-whisper-large-v3"));
}

#[test]
fn models_list_outputs_alias_column() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert!(output.stdout.contains("Alias"));
    assert!(output.stdout.contains("whisper-turbo"));
    assert!(output.stdout.contains("sensevoice"));
}

#[test]
fn models_list_filters_by_keyword_query() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "turbo",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(!output.stdout.contains("sherpa-onnx-whisper-large-v3"));
}

#[test]
fn models_list_filters_by_secondary_alias() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "list",
        "sensevoice-int8",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert!(
        output
            .stdout
            .contains("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
    );
}

#[test]
fn models_verify_reports_not_installed_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "verify",
        "whisper-turbo",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 3);
    assert!(error.to_string().contains("is not installed at"));
}

#[test]
fn models_verify_reports_corrupted_model_files() {
    let dir = tempfile::tempdir().unwrap();
    let install_path = dir.path().join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&install_path).unwrap();
    // Corrupted file (missing required companion files or wrong hashes)
    std::fs::write(install_path.join("corrupted.bin"), b"invalid").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "verify",
        "whisper-turbo",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 3);
    assert!(error.to_string().contains("failed verification"));
    assert!(
        error
            .to_string()
            .contains("Run 'sona-cli models download whisper-turbo' to repair.")
    );
}

#[test]
fn models_path_prints_resolved_directory() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "path",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert_eq!(output.stderr, "");
    assert_eq!(output.stdout.trim(), dir.path().to_string_lossy().as_ref());
}

#[test]
fn models_verify_requires_either_model_id_or_all() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "verify",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn models_verify_rejects_both_model_id_and_all() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "verify",
        "whisper-turbo",
        "--all",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn models_verify_all_reports_no_installed_models_when_empty() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "verify",
        "--all",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert!(output.stdout.contains("No installed models found"));
}

#[test]
fn models_info_text_outputs_full_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "info",
        "whisper-turbo",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(output.stdout.contains("whisper-turbo"));
    assert!(output.stdout.contains("Languages:"));
    assert!(output.stdout.contains("Companions:"));
    assert!(output.stdout.contains("silero-v5-vad"));
    assert!(output.stdout.contains("Artifacts:"));
}

#[test]
fn models_info_json_outputs_valid_schema() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "info",
        "sensevoice",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
        "-j",
    ])
    .unwrap();

    assert_eq!(output.stderr, "");
    let json: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(
        json["id"],
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"
    );
    assert!(
        json["aliases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "sensevoice")
    );
    assert_eq!(json["installed"], false);
    assert!(json["languages"].as_array().unwrap().len() >= 5);
    assert!(!json["artifacts"].as_array().unwrap().is_empty());
}

#[test]
fn models_info_unknown_model_reports_suggestion() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "info",
        "whisper-turb",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("Did you mean"));
}

#[test]
fn models_delete_requires_either_model_id_or_all() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn models_delete_rejects_both_model_id_and_all() {
    let dir = tempfile::tempdir().unwrap();
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "whisper-turbo",
        "--all",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn models_delete_all_empty_directory_reports_none_found() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "--all",
        "--yes",
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("No installed models found in"));
}

#[test]
fn models_delete_all_without_yes_in_noninteractive_fails() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let install_path = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&install_path).unwrap();
    std::fs::write(install_path.join("turbo-encoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(install_path.join("turbo-decoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(install_path.join("turbo-tokens.txt"), b"fake").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "--all",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Cannot prompt for confirmation in non-interactive shell")
    );
}

#[test]
fn models_delete_all_with_yes_deletes_installed_presets_and_preserves_custom_dir() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let preset_install_path = models_dir.join("sherpa-onnx-whisper-turbo");
    std::fs::create_dir_all(&preset_install_path).unwrap();
    std::fs::write(preset_install_path.join("turbo-encoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(preset_install_path.join("turbo-decoder.int8.onnx"), b"fake").unwrap();
    std::fs::write(preset_install_path.join("turbo-tokens.txt"), b"fake").unwrap();

    // An unknown custom directory that is not part of the preset catalog
    let custom_dir = models_dir.join("my-custom-untracked-model");
    std::fs::create_dir_all(&custom_dir).unwrap();
    std::fs::write(custom_dir.join("weights.bin"), "custom").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "models",
        "delete",
        "--all",
        "--yes",
        "--models-dir",
        models_dir.to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert!(output.stderr.contains("Deleted 1 installed model(s)"));
    assert!(output.stderr.contains("sherpa-onnx-whisper-turbo"));
    // The preset model directory should be removed
    assert!(!preset_install_path.exists());
    // The untracked custom directory must be preserved
    assert!(custom_dir.exists());
}
