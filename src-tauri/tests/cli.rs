use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn cli_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sona"));
    command.env("SONA_FORCE_CLI", "1");
    command.env("SONA_TEST_NON_INTERACTIVE_OK", "1");
    command
}

#[test]
fn help_is_printed_to_stdout() {
    let output = cli_command().arg("--help").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sona"));
    assert!(stdout.contains("transcribe"));
    assert!(stdout.contains("models"));
    assert!(stdout.contains("sona models list --type whisper --language zh"));
    assert!(stdout.contains("sona models delete sherpa-onnx-whisper-turbo"));
}

#[test]
fn help_shows_distinct_version_and_verbose_flags() {
    let output = cli_command().arg("--help").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-V, --version"));
    assert!(stdout.contains("-v, --verbose"));
}

#[test]
fn version_is_printed_to_stdout() {
    let output = cli_command().arg("--version").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("sona {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(&expected));
}

#[test]
fn short_help_is_printed_to_stdout() {
    let output = cli_command().arg("-h").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sona"));
    assert!(stdout.contains("transcribe"));
}

#[test]
fn short_version_is_printed_to_stdout() {
    let output = cli_command().arg("-V").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("sona {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(&expected));
}

#[test]
fn short_verbose_does_not_print_version() {
    let output = cli_command()
        .args(["-v", "models", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_version = format!("sona {}", env!("CARGO_PKG_VERSION"));
    assert!(!stdout.contains(&expected_version));
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
}

#[test]
fn transcribe_help_mentions_key_examples() {
    let output = cli_command()
        .args(["transcribe", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--model-id"));
    assert!(stdout.contains("--vad-model-id"));
    assert!(stdout.contains("--gpu-acceleration"));
    assert!(stdout.contains("sample.srt"));
    assert!(stdout.contains("Offline preset model id to use for transcription"));
}

#[test]
fn transcribe_help_mentions_short_config_alias() {
    let output = cli_command()
        .args(["transcribe", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-c, --config"));
}

#[test]
fn transcribe_help_mentions_batch_directory_options() {
    let output = cli_command()
        .args(["transcribe", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--input-dir"));
    assert!(stdout.contains("--output-dir"));
    assert!(stdout.contains("--recursive"));
    assert!(stdout.contains("--jobs"));
}

#[test]
fn transcribe_input_dir_requires_output_dir_before_model_resolution() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sample.wav"), "").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg("--input-dir")
        .arg(dir.path())
        .arg("--model-id")
        .arg("not-a-real-model")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--output-dir"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn transcribe_input_dir_rejects_zero_jobs_before_model_resolution() {
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir_all(&input_dir).unwrap();
    fs::write(input_dir.join("sample.wav"), "").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg("--input-dir")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("0")
        .arg("--model-id")
        .arg("not-a-real-model")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--jobs"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn transcribe_input_dir_rejects_duplicate_output_names_before_model_resolution() {
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir_all(&input_dir).unwrap();
    fs::write(input_dir.join("demo.wav"), "").unwrap();
    fs::write(input_dir.join("demo.mp4"), "").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg("--input-dir")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--model-id")
        .arg("not-a-real-model")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("would overwrite"));
    assert!(stderr.contains("demo.json"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn serve_help_mentions_runtime_defaults() {
    let output = cli_command().args(["serve", "--help"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--config"));
    assert!(stdout.contains("Bind address for the HTTP API server"));
    assert!(stdout.contains("TCP port for the HTTP API server"));
    assert!(stdout.contains("Bearer API key required by HTTP requests"));
    assert!(stdout.contains("Allowed client IP rules"));
    assert!(stdout.contains("--gpu-acceleration"));
    assert!(stdout.contains("--vad-model-id"));
    assert!(stdout.contains("--punctuation-model-id"));
    assert!(stdout.contains("--max-concurrent"));
    assert!(stdout.contains("--max-queue-size"));
    assert!(stdout.contains("--max-upload-size-mb"));
    assert!(stdout.contains("--job-ttl-minutes"));
}

#[test]
fn serve_help_mentions_short_config_alias() {
    let output = cli_command().args(["serve", "--help"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-c, --config"));
}

#[test]
fn init_config_help_mentions_path_and_force() {
    let output = cli_command()
        .args(["init-config", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[PATH]"));
    assert!(stdout.contains("--force"));
    assert!(stdout.contains("sona-cli.toml"));
    assert!(stdout.contains("commented TOML starter template"));
    assert!(stdout.contains("model_id"));
}

#[test]
fn init_config_writes_commented_template_to_target_path() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");

    let output = cli_command()
        .arg("init-config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(stdout.is_empty());
    assert!(stderr.contains("Created config template"));
    assert!(contents.contains("# Sona CLI config template"));
    assert!(contents.contains("# model_id = \"sherpa-onnx-whisper-turbo\""));
    assert!(contents.contains("# api_key = \"\""));
    assert!(contents.contains("# max_upload_size_mb = 50"));
}

#[test]
fn init_config_writes_default_sona_cli_toml_in_current_dir() {
    let dir = tempdir().unwrap();

    let output = cli_command()
        .arg("init-config")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());

    let config_path = dir.path().join("sona-cli.toml");
    let contents = fs::read_to_string(config_path).unwrap();
    assert!(contents.contains("# Sona CLI config template"));
}

#[test]
fn init_config_rejects_existing_target_without_force() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    fs::write(&config_path, "existing = true\n").unwrap();

    let output = cli_command()
        .arg("init-config")
        .arg(&config_path)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(
        fs::read_to_string(&config_path).unwrap(),
        "existing = true\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--force"));
}

#[test]
fn init_config_force_overwrites_existing_target() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    fs::write(&config_path, "existing = true\n").unwrap();

    let output = cli_command()
        .arg("init-config")
        .arg(&config_path)
        .arg("--force")
        .output()
        .unwrap();

    assert!(output.status.success());
    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(!contents.contains("existing = true"));
    assert!(contents.contains("[transcribe]"));
    assert!(contents.contains("[serve]"));
}

#[test]
fn serve_invalid_gpu_acceleration_returns_failure() {
    let output = cli_command()
        .args(["serve", "--gpu-acceleration", "vulkan"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gpu_acceleration"));
    assert!(stderr.contains("auto, cpu, cuda, coreml, directml"));
}

#[test]
fn serve_invalid_ip_whitelist_returns_failure() {
    let output = cli_command()
        .args(["serve", "--ip-whitelist", "not-an-ip-rule"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to parse IP whitelist"));
    assert!(stderr.contains("not-an-ip-rule"));
}

#[test]
fn models_help_mentions_list_download_and_delete() {
    let output = cli_command().args(["models", "--help"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("download"));
    assert!(stdout.contains("delete"));
}

#[test]
fn models_list_help_mentions_new_filters() {
    let output = cli_command()
        .args(["models", "list", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--type"));
    assert!(stdout.contains("--language"));
    assert!(stdout.contains("--installed"));
    assert!(stdout.contains("sona models list --mode offline --type whisper"));
}

#[test]
fn models_download_help_mentions_companions() {
    let output = cli_command()
        .args(["models", "download", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("silero-vad"));
    assert!(stdout.contains("--models-dir"));
}

#[test]
fn models_delete_help_mentions_confirmation() {
    let output = cli_command()
        .args(["models", "delete", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<MODEL_ID>"));
    assert!(stdout.contains("--models-dir"));
    assert!(stdout.contains("--yes"));
}

#[test]
fn model_list_outputs_table_by_default() {
    let output = cli_command().args(["models", "list"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ID"));
    assert!(stdout.contains("Type"));
    assert!(stdout.contains("Language"));
    assert!(stdout.contains("Installed"));
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("silero-vad"));
    assert!(!stdout.contains("\"installed\""));
}

#[test]
fn model_list_outputs_json_with_json_flag() {
    let output = cli_command()
        .args(["models", "list", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"installed\""));
    assert!(stdout.contains("\"install_path\""));
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
}

#[test]
fn completions_powershell_prints_script() {
    let output = cli_command()
        .args(["completions", "powershell"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Register-ArgumentCompleter"));
    assert!(stdout.contains("sona"));
}

#[test]
fn model_list_can_filter_by_mode() {
    let output = cli_command()
        .args(["models", "list", "--mode", "streaming"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30"));
    assert!(!stdout.contains("sherpa-onnx-whisper-turbo"));
}

#[test]
fn model_list_can_filter_by_type() {
    let output = cli_command()
        .args(["models", "list", "--type", "whisper"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("sherpa-onnx-whisper-large-v3"));
    assert!(!stdout.contains("silero-vad"));
}

#[test]
fn model_list_can_filter_by_language() {
    let output = cli_command()
        .args(["models", "list", "--language", "yue"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"));
    assert!(!stdout.contains("silero-vad"));
}

#[test]
fn model_list_can_filter_by_type_and_language() {
    let output = cli_command()
        .args(["models", "list", "--type", "whisper", "--language", "zh"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("sherpa-onnx-whisper-large-v3"));
    assert!(!stdout.contains("silero-vad"));
    assert!(!stdout.contains("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30"));
}

#[test]
fn model_list_can_filter_installed_only() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

    let output = cli_command()
        .arg("models")
        .arg("list")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--installed")
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(!stdout.contains("silero-vad"));
}

#[test]
fn missing_config_file_returns_failure() {
    let output = cli_command()
        .args([
            "transcribe",
            "sample.wav",
            "--config",
            "does-not-exist.toml",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to read config file"));
}

#[test]
fn unknown_model_id_returns_failure() {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("sample.wav");
    fs::write(&input_path, "").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg(&input_path)
        .args([
            "--models-dir",
            "C:\\models",
            "--model-id",
            "not-a-real-model",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
}

#[test]
fn missing_single_file_input_returns_failure_before_model_resolution() {
    let dir = tempdir().unwrap();
    let missing_input = dir.path().join("missing.wav");

    let output = cli_command()
        .arg("transcribe")
        .arg(&missing_input)
        .args(["--model-id", "not-a-real-model"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Input file must be an existing file"));
    assert!(stderr.contains(&missing_input.display().to_string()));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn existing_single_output_returns_failure_before_model_resolution_without_force() {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("sample.wav");
    let output_path = dir.path().join("sample.srt");
    fs::write(&input_path, "").unwrap();
    fs::write(&output_path, "existing").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .args(["--model-id", "not-a-real-model"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Output file already exists"));
    assert!(stderr.contains(&output_path.display().to_string()));
    assert!(stderr.contains("--force"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn existing_single_output_with_force_reaches_model_resolution() {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("sample.wav");
    let output_path = dir.path().join("sample.srt");
    fs::write(&input_path, "").unwrap();
    fs::write(&output_path, "existing").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg(&input_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--force")
        .args(["--model-id", "not-a-real-model"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
    assert!(!stderr.contains("Output file already exists"));
}

#[test]
fn existing_batch_output_returns_failure_before_model_resolution_without_force() {
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir_all(&input_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(input_dir.join("sample.wav"), "").unwrap();
    fs::write(output_dir.join("sample.json"), "existing").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg("--input-dir")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .args(["--model-id", "not-a-real-model"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Output file already exists"));
    assert!(stderr.contains("sample.json"));
    assert!(stderr.contains("--force"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn multiple_positional_inputs_require_output_dir_before_model_resolution() {
    let dir = tempdir().unwrap();
    let first = dir.path().join("first.wav");
    let second = dir.path().join("second.wav");
    fs::write(&first, "").unwrap();
    fs::write(&second, "").unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg(&first)
        .arg(&second)
        .args(["--model-id", "not-a-real-model"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--output-dir"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn glob_inputs_require_output_dir_before_model_resolution() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("first.wav"), "").unwrap();
    fs::write(dir.path().join("second.wav"), "").unwrap();
    let pattern = dir.path().join("*.wav").to_string_lossy().to_string();

    let output = cli_command()
        .arg("transcribe")
        .arg(pattern)
        .args(["--model-id", "not-a-real-model"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--output-dir"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn invalid_gpu_acceleration_returns_failure_before_model_resolution() {
    let output = cli_command()
        .args([
            "transcribe",
            "sample.wav",
            "--gpu-acceleration",
            "vulkan",
            "--model-id",
            "not-a-real-model",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gpu_acceleration"));
    assert!(stderr.contains("auto, cpu, cuda, coreml, directml"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn unknown_download_model_id_returns_failure() {
    let output = cli_command()
        .args(["models", "download", "not-a-real-model", "--quiet"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
}

#[test]
fn unknown_delete_model_id_returns_failure() {
    let output = cli_command()
        .args(["models", "delete", "not-a-real-model", "--yes"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
}

#[test]
fn delete_installed_archive_model_with_yes_removes_only_target() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_path = models_dir.join("sherpa-onnx-whisper-turbo");
    let vad_path = models_dir.join("silero_vad.onnx");
    fs::create_dir_all(&model_path).unwrap();
    fs::write(model_path.join("model.onnx"), "").unwrap();
    fs::write(&vad_path, "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("delete")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!model_path.exists());
    assert!(vad_path.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Deleted sherpa-onnx-whisper-turbo"));
}

#[test]
fn delete_installed_single_file_model_with_yes_removes_file() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(&models_dir).unwrap();
    let vad_path = models_dir.join("silero_vad.onnx");
    fs::write(&vad_path, "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("delete")
        .arg("silero-vad")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!vad_path.exists());
}

#[test]
fn delete_known_missing_model_succeeds_with_notice() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let output = cli_command()
        .arg("models")
        .arg("delete")
        .arg("silero-vad")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Model silero-vad is not installed"));
}

#[test]
fn delete_interactive_confirmation_decline_leaves_model() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_path = models_dir.join("sherpa-onnx-whisper-turbo");
    fs::create_dir_all(&model_path).unwrap();

    let mut child = cli_command()
        .arg("models")
        .arg("delete")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"n\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(model_path.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Delete cancelled"));
}

#[test]
fn delete_interactive_confirmation_accepts_y() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_path = models_dir.join("sherpa-onnx-whisper-turbo");
    fs::create_dir_all(&model_path).unwrap();

    let mut child = cli_command()
        .arg("models")
        .arg("delete")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"y\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(!model_path.exists());
}

#[test]
fn repeated_download_of_installed_model_also_installs_required_vad() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();
    fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("download")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--quiet")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Installed sherpa-onnx-whisper-turbo"));
    assert!(stderr.contains("Installed silero-vad"));
}

#[test]
fn missing_default_vad_model_returns_failure() {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("sample.wav");
    fs::write(&input_path, "").unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg(&input_path)
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--model-id")
        .arg("sherpa-onnx-whisper-turbo")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Companion model 'silero-vad' was not found"));
}
