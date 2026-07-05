#[test]
fn transcribe_command_is_exposed_by_standalone_cli() {
    let error = sona_cli::run_cli_from_args(["sona-cli", "transcribe", "--help"]).unwrap_err();
    let output = error.to_string();
    assert!(output.contains("transcribe"));
    assert!(output.contains("Usage:"));
}
