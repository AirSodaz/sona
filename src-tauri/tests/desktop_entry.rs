use std::process::Command;

fn desktop_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sona"));
    command.env("SONA_TEST_EXIT_BEFORE_APP", "1");
    command
}

#[test]
fn desktop_binary_ignores_legacy_cli_force_flag() {
    let output = desktop_command()
        .env("SONA_FORCE_CLI", "1")
        .args(["transcribe", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "desktop-entry"
    );
}

#[test]
fn desktop_binary_ignores_legacy_cli_subcommands() {
    let output = desktop_command().args(["models", "list"]).output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "desktop-entry"
    );
}
