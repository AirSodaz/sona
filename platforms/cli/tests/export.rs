use std::fs;

use serde_json::Value;

fn write_segments(path: &std::path::Path) {
    fs::write(
        path,
        serde_json::to_vec_pretty(&serde_json::json!([{
            "id": "segment-1",
            "text": "Hello",
            "start": 0.0,
            "end": 1.25,
            "isFinal": true,
            "translation": "Bonjour"
        }]))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn export_transcript_writes_requested_format_and_returns_json_result() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.vtt");
    write_segments(&input);

    let result = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "--input",
        input.to_string_lossy().as_ref(),
        "--output",
        output.to_string_lossy().as_ref(),
        "--format",
        "vtt",
        "--mode",
        "bilingual",
        "--json",
    ])
    .unwrap();

    let report: Value = serde_json::from_str(&result.stdout).unwrap();
    let content = fs::read_to_string(&output).unwrap();
    assert_eq!(result.stderr, "");
    assert_eq!(report["outputPath"], output.to_string_lossy().as_ref());
    assert_eq!(report["bytesWritten"], content.len() as u64);
    assert!(content.starts_with("WEBVTT"));
    assert!(content.contains("Bonjour\nHello"));
}

#[test]
fn export_transcript_infers_format_and_defaults_to_original_table_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.txt");
    write_segments(&input);

    let result = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "--input",
        input.to_string_lossy().as_ref(),
        "--output",
        output.to_string_lossy().as_ref(),
    ])
    .unwrap();

    assert_eq!(fs::read_to_string(&output).unwrap(), "Hello");
    assert_eq!(result.stdout.lines().count(), 3);
    assert!(result.stdout.lines().next().unwrap().contains("OUTPUT"));
    assert!(result.stdout.lines().next().unwrap().contains("BYTES"));
}

#[test]
fn export_transcript_rejects_invalid_json_before_writing_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("invalid.json");
    let output = dir.path().join("transcript.txt");
    fs::write(&input, [0xff_u8, 0xfe]).unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "--input",
        input.to_string_lossy().as_ref(),
        "--output",
        output.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Validation(_)));
    assert_eq!(error.exit_code(), 2);
    assert!(!output.exists());
}

#[test]
fn export_transcript_rejects_unknown_format_before_writing_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.txt");
    write_segments(&input);

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "--input",
        input.to_string_lossy().as_ref(),
        "--output",
        output.to_string_lossy().as_ref(),
        "--format",
        "docx",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Validation(_)));
    assert!(!output.exists());
}
