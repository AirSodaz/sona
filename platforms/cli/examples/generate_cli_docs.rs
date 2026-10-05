use clap::Command;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn generate_command_markdown(cmd: &Command, parent_prefix: &str, depth: usize) -> String {
    let mut out = String::new();
    let name = cmd.get_name();
    let full_name = if parent_prefix.is_empty() {
        name.to_string()
    } else {
        format!("{parent_prefix} {name}")
    };

    let heading_prefix = "#".repeat(depth.min(6));
    let about = cmd.get_about().map(|a| a.to_string()).unwrap_or_default();
    let aliases = cmd.get_visible_aliases().collect::<Vec<_>>();

    let _ = writeln!(out, "{heading_prefix} `{full_name}`\n");
    if !about.is_empty() {
        let _ = writeln!(out, "{about}\n");
    }
    if !aliases.is_empty() {
        let _ = writeln!(
            out,
            "**Aliases:** {}\n",
            aliases
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Arguments table
    let visible_args: Vec<_> = cmd
        .get_arguments()
        .filter(|a| !a.is_hide_set() && a.get_id() != "help" && a.get_id() != "version")
        .collect();

    if !visible_args.is_empty() {
        let _ = writeln!(out, "**Options & Arguments:**\n");
        let _ = writeln!(out, "| Option / Argument | Short | Default | Description |");
        let _ = writeln!(out, "| --- | --- | --- | --- |");

        for arg in visible_args {
            let mut name_part = String::new();
            if let Some(long) = arg.get_long() {
                name_part.push_str(&format!("`--{long}`"));
            } else if let Some(val_name) = arg.get_value_names().and_then(|v| v.first()) {
                name_part.push_str(&format!("`<{val_name}>`"));
            } else {
                name_part.push_str(&format!("`{}`", arg.get_id()));
            }

            let short_part = arg
                .get_short()
                .map(|s| format!("`-{s}`"))
                .unwrap_or_else(|| "-".to_string());

            let default_part = arg
                .get_default_values()
                .first()
                .map(|d| format!("`{}`", d.to_string_lossy()))
                .unwrap_or_else(|| "-".to_string());

            let help_part = arg
                .get_help()
                .map(|h| h.to_string().replace('|', "\\|").replace('\n', " "))
                .unwrap_or_default();

            let _ = writeln!(
                out,
                "| {name_part} | {short_part} | {default_part} | {help_part} |"
            );
        }
        out.push('\n');
    }

    // Examples / After help
    if let Some(after_help) = cmd.get_after_help() {
        let text = after_help.to_string().trim().to_string();
        if !text.is_empty() {
            let _ = writeln!(out, "**Usage & Examples:**\n");
            let _ = writeln!(out, "```text\n{text}\n```\n");
        }
    }

    // Recurse public subcommands
    let mut subcommands: Vec<_> = cmd
        .get_subcommands()
        .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        .collect();
    subcommands.sort_by_key(|s| s.get_name());

    for sub in subcommands {
        out.push_str(&generate_command_markdown(sub, &full_name, depth + 1));
    }

    out
}

fn generate_hidden_commands_markdown(root: &Command) -> String {
    let mut out = String::new();
    let mut hidden_subs: Vec<_> = root
        .get_subcommands()
        .filter(|s| s.is_hide_set() && s.get_name() != "help")
        .collect();
    hidden_subs.sort_by_key(|s| s.get_name());

    if hidden_subs.is_empty() {
        return out;
    }

    out.push_str("## Internal Integration Contracts (Hidden Commands)\n\n");
    out.push_str("The following commands are hidden from `--help` by default and are intended for host/desktop integration, facts snapshot construction, and automated path contract inspection:\n\n");

    for sub in hidden_subs {
        out.push_str(&generate_command_markdown(sub, "sona-cli", 3));
    }

    out
}

pub fn generate_cli_markdown() -> String {
    let root = sona_cli::cli_command();
    let mut doc = String::new();

    doc.push_str("# Sona CLI Guide & Reference\n\n");
    doc.push_str(
        "<!-- Generated automatically from `sona_cli::cli_command()`. DO NOT EDIT MANUALLY. -->\n",
    );
    doc.push_str("<!-- To regenerate or verify: `cargo run -p sona-cli --example generate_cli_docs [-- --check]` (or `pnpm run generate:cli-docs`) -->\n\n");
    doc.push_str("`sona-cli` is a standalone, stateless command-line speech-to-text host backed by `sona-core`. It transcribes audio files, directories, live microphone input, and standard input streams with local preset ASR models or online cloud providers.\n\n");

    doc.push_str("## Stateless Boundary\n\n");
    doc.push_str("The CLI deliberately excludes SQLite, History, Tag, application backup/recovery, Sync, and Online LLM tasks. It will not silently create or mutate desktop application data directories. Output is emitted to stdout or written to explicitly specified target files.\n\n");

    doc.push_str("## Configuration Precedence\n\n");
    doc.push_str("Configuration options are resolved in the following priority order:\n\n");
    doc.push_str("1. Explicit CLI argument: `-c / --config <PATH>`;\n");
    doc.push_str("2. Global environment variable: `SONA_CONFIG` (if set and non-empty);\n");
    doc.push_str("3. Local configuration file in current working directory: `./sona-cli.toml` (if the file exists);\n");
    doc.push_str("4. User standard configuration file: `sona-cli.toml` (if the file exists; Linux: `~/.config/sona/sona-cli.toml`, macOS: `~/Library/Application Support/sona/sona-cli.toml`, Windows: `%APPDATA%\\sona\\sona-cli.toml` or `%USERPROFILE%\\.config\\sona\\sona-cli.toml`);\n");
    doc.push_str("5. Built-in defaults: if none of the above files exist, the CLI runs with built-in defaults without error.\n\n");

    doc.push_str("## Exit Codes & Errors\n\n");
    doc.push_str("- `0`: Success;\n");
    doc.push_str(
        "- `1`: General failure (e.g. `doctor --strict` health check failure, serialization error);\n",
    );
    doc.push_str(
        "- `2`: Validation or CLI usage error (invalid argument, missing provider, nonexistent input file, duration <= 0);\n",
    );
    doc.push_str(
        "- `3`: Model error (missing preset, corrupted download, uninstalled companion);\n",
    );
    doc.push_str(
        "- `4`: Network or online provider error (authentication failure, API timeout);\n",
    );
    doc.push_str(
        "- `5`: Filesystem error (output file already exists without `--force`, directory write failure);\n",
    );
    doc.push_str("- `130`: Cancelled (interrupted by Ctrl-C signal).\n\n");

    doc.push_str("## Public Command Reference\n\n");
    doc.push_str(&generate_command_markdown(&root, "", 3));

    let hidden = generate_hidden_commands_markdown(&root);
    if !hidden.is_empty() {
        doc.push_str(&hidden);
    }

    // Ensure exactly one trailing newline without empty line at EOF
    let mut final_doc = doc.trim_end().to_string();
    final_doc.push('\n');
    final_doc
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn main() -> ExitCode {
    let check_mode = std::env::args().any(|arg| arg == "--check");
    let target_path = repo_root().join("docs").join("cli.md");
    let generated = generate_cli_markdown();

    if check_mode {
        match std::fs::read_to_string(&target_path) {
            Ok(existing) => {
                // Normalize CRLF to LF for comparison
                let norm_existing = existing.replace("\r\n", "\n");
                let norm_generated = generated.replace("\r\n", "\n");
                if norm_existing == norm_generated {
                    println!("OK: docs/cli.md is up to date with CLI definitions.");
                    ExitCode::SUCCESS
                } else {
                    eprintln!("ERROR: docs/cli.md is out of date with CLI definitions.");
                    eprintln!(
                        "Run 'cargo run -p sona-cli --example generate_cli_docs' (or 'pnpm run generate:cli-docs') to update."
                    );
                    ExitCode::FAILURE
                }
            }
            Err(err) => {
                eprintln!("ERROR: Failed to read {}: {err}", target_path.display());
                ExitCode::FAILURE
            }
        }
    } else {
        if let Some(parent) = target_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&target_path, &generated) {
            Ok(()) => {
                println!(
                    "Successfully generated CLI documentation at {}",
                    target_path.display()
                );
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("ERROR: Failed to write {}: {err}", target_path.display());
                ExitCode::FAILURE
            }
        }
    }
}
