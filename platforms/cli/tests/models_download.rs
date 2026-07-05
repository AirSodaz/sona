#[test]
fn models_download_help_mentions_companion_downloads() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "models", "download", "--help"]).unwrap_err();
    let message = error.to_string();

    assert!(message.contains("Download"));
    assert!(message.contains("--models-dir"));
    assert!(message.contains("--quiet"));
    assert!(message.contains("Preset model id"));
}
