use std::sync::Mutex;

static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn init_config_writes_commented_template_to_target_path() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "init-config",
        config_path.to_string_lossy().as_ref(),
    ])
    .unwrap();

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Created config template"));
    assert!(contents.contains("# Sona CLI config template"));
    assert!(contents.contains("# model_id = \"sherpa-onnx-whisper-turbo\""));
    assert!(contents.contains("[transcribe]"));
    assert!(!contents.contains("# api_key = \"\""));
    assert!(!contents.contains("[serve]"));
    assert!(!contents.contains("sona-cli serve"));
}

#[test]
fn init_config_writes_default_sona_cli_toml_in_current_dir() {
    let _guard = CURRENT_DIR_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = sona_cli::run_cli_from_args(["sona-cli", "init-config"]);

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
        "init-config",
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
        "init-config",
        config_path.to_string_lossy().as_ref(),
        "--force",
    ])
    .unwrap();

    let contents = std::fs::read_to_string(config_path).unwrap();
    assert_eq!(output.stdout, "");
    assert!(output.stderr.contains("Created config template"));
    assert!(!contents.contains("existing = true"));
    assert!(contents.contains("[transcribe]"));
    assert!(!contents.contains("[serve]"));
    assert!(!contents.contains("sona-cli serve"));
}
