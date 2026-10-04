use std::sync::Mutex;

static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn init_config_writes_commented_template_to_target_path() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "init",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Created config template"));
    assert!(contents.contains("# Sona CLI config template"));
    assert!(contents.contains("# model_id = \"sherpa-onnx-whisper-turbo\""));
    assert!(contents.contains("# api_key_env = \"SONA_VOLCENGINE_ASR_API_KEY\""));
    assert!(!contents.contains("# api_key_env = \"VOLCENGINE_API_KEY\""));
    assert!(contents.contains("[transcribe]"));
    assert!(contents.contains("[transcribe_live]"));
    assert!(contents.contains("# input = \"microphone\""));
    assert!(contents.contains("# output_format = \"text\""));
    assert!(contents.contains("# api_key = \"\""));
    assert!(contents.contains("[serve]"));
    assert!(contents.contains("sona-cli serve"));
}

#[test]
fn init_config_writes_default_sona_cli_toml_in_current_dir() {
    let _guard = CURRENT_DIR_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = sona_cli::run_cli_from_args(["sona-cli", "config", "init"]);

    std::env::set_current_dir(original_dir).unwrap();
    let output = result.unwrap();
    let contents = std::fs::read_to_string(dir.path().join("sona-cli.toml")).unwrap();

    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Created config template"));
    assert!(contents.contains("# Sona CLI config template"));
}

#[test]
fn init_config_rejects_existing_target_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(&config_path, "existing = true\n").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "init",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 5);
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "existing = true\n"
    );
    assert!(error.to_string().contains("--force"));
}

#[test]
fn init_config_force_overwrites_existing_target() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(&config_path, "existing = true\n").unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "init",
        config_path.to_string_lossy().as_ref(),
        "--force",
    ])
    .unwrap();

    let contents = std::fs::read_to_string(config_path).unwrap();
    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Created config template"));
    assert!(!contents.contains("existing = true"));
    assert!(contents.contains("[transcribe]"));
    assert!(contents.contains("[serve]"));
    assert!(contents.contains("sona-cli serve"));
}

#[test]
fn auto_discovers_sona_cli_toml_in_current_dir() {
    let _guard = CURRENT_DIR_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let config_content = "[transcribe]\nmodel_id = \"whisper-turbo\"\n";
    std::fs::write(dir.path().join("sona-cli.toml"), config_content).unwrap();

    // Fake an audio file so we get past the input file existence check
    let fake_audio = dir.path().join("sample.wav");
    std::fs::write(&fake_audio, b"fake").unwrap();

    // Running transcribe WITHOUT -c/--config should pick up model_id from ./sona-cli.toml
    // and fail at model-not-found-on-disk (meaning config was successfully loaded!),
    // rather than "Missing required batch model"
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "transcribe",
        fake_audio.to_string_lossy().as_ref(),
        "--models-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    std::env::set_current_dir(original_dir).unwrap();

    // If config was loaded, model_id was set to whisper-turbo, so error will say model was not found
    assert!(
        error.to_string().contains("sherpa-onnx-whisper-turbo")
            || error.to_string().contains("was not found"),
        "Expected model not found error, got: {error}"
    );
}

#[test]
fn config_subcommands_work() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("my-config.toml");

    // 1. config init
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "init",
        target.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert!(output.stderr.contains("Created config template"));
    assert!(target.is_file());

    // 2. config path
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "path",
        "-c",
        target.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(output.stdout.trim(), target.display().to_string());

    // 3. config check on valid template
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "check",
        "-c",
        target.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert!(output.stdout.contains("is valid"));

    // 4. config show
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "show",
        "-c",
        target.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert!(output.stdout.contains("[transcribe]"));
    assert!(output.stdout.contains("[serve]"));

    // 4b. config get
    let sample_cfg = dir.path().join("sample_get.toml");
    std::fs::write(
        &sample_cfg,
        "[transcribe]\nmodel_id = \"whisper-turbo\"\nthreads = 4\nenable_itn = true\n",
    )
    .unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.model_id",
        "-c",
        sample_cfg.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(output.stdout.trim(), "whisper-turbo");

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.threads",
        "-c",
        sample_cfg.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(output.stdout.trim(), "4");

    let err = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.nonexistent_key",
        "-c",
        sample_cfg.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(err.to_string().contains("not found"));

    // 5. config check on invalid toml
    let bad = dir.path().join("bad.toml");
    std::fs::write(&bad, "invalid = [[[").unwrap();
    let err = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "check",
        "-c",
        bad.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(err.to_string().contains("Configuration error"));
}

#[test]
fn init_config_rejects_conflicting_path_and_global_flags() {
    let error1 =
        sona_cli::run_cli_from_args(["sona-cli", "config", "init", "custom.toml", "--global"])
            .unwrap_err();
    assert!(matches!(error1, sona_cli::CliError::Usage(_)));

    let error2 =
        sona_cli::run_cli_from_args(["sona-cli", "config", "init", "custom.toml", "--user"])
            .unwrap_err();
    assert!(matches!(error2, sona_cli::CliError::Usage(_)));
}

#[test]
fn init_config_global_writes_to_isolated_subprocess_user_path() {
    let dir = tempfile::tempdir().unwrap();
    let work_dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_sona-cli");
    let mut cmd = std::process::Command::new(bin);
    cmd.current_dir(work_dir.path());
    cmd.env_remove("SONA_CONFIG");

    #[cfg(target_os = "windows")]
    {
        cmd.env("APPDATA", dir.path());
        cmd.env_remove("USERPROFILE");
        cmd.env_remove("LOCALAPPDATA");
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd.env("XDG_CONFIG_HOME", dir.path());
        cmd.env_remove("HOME");
    }

    let output = cmd
        .args(["config", "init", "--global"])
        .output()
        .expect("failed to execute sona-cli binary");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Created config template"));

    let expected_file = dir.path().join("sona").join("sona-cli.toml");
    assert!(expected_file.is_file());

    // Also test alias --user with --force
    let mut cmd_user = std::process::Command::new(bin);
    cmd_user.current_dir(work_dir.path());
    cmd_user.env_remove("SONA_CONFIG");
    #[cfg(target_os = "windows")]
    {
        cmd_user.env("APPDATA", dir.path());
        cmd_user.env_remove("USERPROFILE");
        cmd_user.env_remove("LOCALAPPDATA");
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd_user.env("XDG_CONFIG_HOME", dir.path());
        cmd_user.env_remove("HOME");
    }

    let output_user = cmd_user
        .args(["config", "init", "--user", "-F"])
        .output()
        .expect("failed to execute sona-cli binary");

    assert!(output_user.status.success());
}

#[test]
fn config_set_and_get_preserves_comments_and_updates_values() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");

    // 1. Initialize template
    sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "init",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    // 2. Set string, int, and bool
    let set_out = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe.model_id",
        "whisper-turbo",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert!(
        set_out
            .stdout
            .contains("Set transcribe.model_id = whisper-turbo")
    );

    sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "serve.port",
        "14200",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe.enable_itn",
        "true",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    // 3. Get values
    let get_model = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.model_id",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_model.stdout.trim(), "whisper-turbo");

    let get_port = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "serve.port",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_port.stdout.trim(), "14200");

    let get_itn = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.enable_itn",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_itn.stdout.trim(), "true");

    // 4. Verify comments were preserved in the file
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("# Sona CLI config template"));
    assert!(content.contains("# api_key_env = \"SONA_VOLCENGINE_ASR_API_KEY\""));
}

#[test]
fn config_set_creates_file_if_missing() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nested").join("custom.toml");

    let set_out = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe.model_id",
        "sensevoice",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert!(
        set_out
            .stdout
            .contains("Set transcribe.model_id = sensevoice")
    );
    assert!(config_path.is_file());

    let get_out = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.model_id",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_out.stdout.trim(), "sensevoice");
}

#[test]
fn config_set_rejects_invalid_type_and_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(&config_path, "[transcribe]\nthreads = 4\n").unwrap();

    let err = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe.threads",
        "not_an_integer",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(err.exit_code(), 2);
    assert!(
        err.to_string()
            .contains("resulting configuration is invalid")
    );

    // Ensure file was rolled back
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(content, "[transcribe]\nthreads = 4\n");
}

#[test]
fn config_set_rejects_invalid_type_on_empty_file_and_retains_it() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(&config_path, b"").unwrap();
    assert!(config_path.is_file());

    let err = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe.threads",
        "not_an_integer",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert_eq!(err.exit_code(), 2);
    assert!(
        config_path.is_file(),
        "0-byte file should not be deleted on rollback"
    );
    assert_eq!(std::fs::metadata(&config_path).unwrap().len(), 0);
}

#[test]
fn config_set_and_get_float_and_live_format_fields() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");

    sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe.vad_buffer_size",
        "0.5",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    let get_vad = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe.vad_buffer_size",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_vad.stdout.trim(), "0.5");

    sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe_live.stream_format",
        "ndjson",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    let get_stream = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe_live.stream_format",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_stream.stdout.trim(), "ndjson");

    sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "set",
        "transcribe_live.format",
        "vtt",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    let get_fmt = sona_cli::run_cli_from_args([
        "sona-cli",
        "config",
        "get",
        "transcribe_live.format",
        "-c",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    assert_eq!(get_fmt.stdout.trim(), "vtt");
}

#[test]
fn config_set_and_get_global_via_isolated_subprocess() {
    let dir = tempfile::tempdir().unwrap();
    let work_dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_sona-cli");

    let mut cmd_set = std::process::Command::new(bin);
    cmd_set.current_dir(work_dir.path());
    cmd_set.env_remove("SONA_CONFIG");
    #[cfg(target_os = "windows")]
    {
        cmd_set.env("APPDATA", dir.path());
        cmd_set.env_remove("USERPROFILE");
        cmd_set.env_remove("LOCALAPPDATA");
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd_set.env("XDG_CONFIG_HOME", dir.path());
        cmd_set.env_remove("HOME");
    }

    let output_set = cmd_set
        .args([
            "config",
            "set",
            "transcribe.model_id",
            "whisper-turbo",
            "--global",
        ])
        .output()
        .expect("failed to execute config set --global");
    assert!(output_set.status.success());

    let mut cmd_get = std::process::Command::new(bin);
    cmd_get.current_dir(work_dir.path());
    cmd_get.env_remove("SONA_CONFIG");
    #[cfg(target_os = "windows")]
    {
        cmd_get.env("APPDATA", dir.path());
        cmd_get.env_remove("USERPROFILE");
        cmd_get.env_remove("LOCALAPPDATA");
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd_get.env("XDG_CONFIG_HOME", dir.path());
        cmd_get.env_remove("HOME");
    }

    let output_get = cmd_get
        .args(["config", "get", "transcribe.model_id", "--global"])
        .output()
        .expect("failed to execute config get --global");
    assert!(output_get.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output_get.stdout).trim(),
        "whisper-turbo"
    );
}
