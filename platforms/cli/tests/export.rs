use std::fs;

use sona_core::export::ExportTranscriptFileResult;

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

    let report: ExportTranscriptFileResult = serde_json::from_str(&result.stdout).unwrap();
    let content = fs::read_to_string(&output).unwrap();
    assert_eq!(result.stderr, "");
    assert_eq!(report.output_path, output.to_string_lossy().as_ref());
    assert_eq!(report.bytes_written, content.len() as u64);
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

    assert_eq!(error.exit_code(), 2);
    assert!(matches!(
        error,
        sona_cli::CliError::Usage(_) | sona_cli::CliError::Validation(_)
    ));
    assert!(!output.exists());
}

#[test]
fn export_transcript_from_stdin_to_stdout_pipes_formatted_content() {
    let json_bytes = serde_json::to_vec_pretty(&serde_json::json!([
        {
            "id": "segment-1",
            "text": "Hello world",
            "start": 0.0,
            "end": 2.5,
            "isFinal": true,
            "translation": "Bonjour monde"
        }
    ]))
    .unwrap();

    let output = sona_cli::run_cli_from_args_with_stdin(
        ["sona-cli", "export", "transcript", "-f", "srt"],
        json_bytes,
    )
    .expect("streaming export to stdout should succeed");

    assert_eq!(output.stderr, "");
    assert!(output.stdout.contains("00:00:00,000 --> 00:00:02,500"));
    assert!(output.stdout.contains("Hello world"));
}

#[test]
fn export_transcript_from_file_to_stdout_pipes_formatted_content() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    write_segments(&input);

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "-i",
        input.to_str().unwrap(),
        "-o",
        "-",
        "-f",
        "vtt",
    ])
    .expect("export to stdout should succeed");

    assert_eq!(output.stderr, "");
    assert!(output.stdout.starts_with("WEBVTT"));
    assert!(output.stdout.contains("Hello"));
}

#[test]
fn export_transcript_rejects_stdout_when_format_is_missing() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "export", "transcript", "-o", "-"]).unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("Export format must be specified with -f/--format when exporting to stdout")
    );
}

#[test]
fn export_direct_without_subcommand_works() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.srt");
    write_segments(&input);

    // Invoking `sona-cli export` directly without typing `transcript`
    let result = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "-i",
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ])
    .expect("direct export without subcommand should succeed");

    assert_eq!(result.stderr, "");
    assert!(output.exists());
    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains("Hello"));
}

#[test]
fn export_transcript_stdin_terminal_returns_clear_validation_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.srt");

    let error = sona_cli::run_cli_from_args_with_terminal_stdin([
        "sona-cli",
        "export",
        "transcript",
        "-o",
        output.to_str().unwrap(),
        "-f",
        "srt",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
    assert!(
        error
            .to_string()
            .contains("No transcript input provided via stdin")
    );
}

#[test]
fn export_transcript_rejects_unknown_mode() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.srt");
    write_segments(&input);

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "-i",
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "-m",
        "invalid_mode",
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 2);
}

#[test]
fn export_transcript_rejects_existing_output_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.srt");
    write_segments(&input);
    fs::write(&output, "already exists").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "-i",
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ])
    .unwrap_err();

    assert_eq!(error.exit_code(), 5);
    assert!(error.to_string().contains("already exists"));
    assert!(error.to_string().contains("Use --force"));
}

#[test]
fn export_transcript_overwrites_existing_output_with_force() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("segments.json");
    let output = dir.path().join("transcript.srt");
    write_segments(&input);
    fs::write(&output, "already exists").unwrap();

    let result = sona_cli::run_cli_from_args([
        "sona-cli",
        "export",
        "transcript",
        "-i",
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
        "--force",
    ])
    .unwrap();

    assert!(result.stdout.contains("transcript.srt"));
    let content = fs::read_to_string(&output).unwrap();
    assert!(content.contains("Hello"));
}
