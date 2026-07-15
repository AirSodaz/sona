use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;
use sona_archive::FsBackupArchiveRepository;
use sona_core::backup::{
    BackupError, BackupExportRequest, BackupImportRequest, BackupInspectRequest, BackupService,
};
use sona_runtime_fs::SystemClock;
use sona_sqlite::LazySqliteBackupStateRepository;

use crate::{CliError, CliOutput, CliResult};

const BACKUP_HELP: &str = r#"Examples:
  sona-cli backup export --app-data-dir ./sona-data --output ./sona-backup.sona-backup --app-version 0.8.0
  sona-cli backup inspect --archive ./sona-backup.sona-backup
  sona-cli backup import --app-data-dir ./sona-data --archive ./sona-backup.sona-backup --default-rule-set-name "Default Rules" --confirm-replace

Warning: import atomically replaces the config, workspace, history, automation, and analytics scopes. It requires --confirm-replace and never opens an interactive prompt. Task ledger and original audio are not included."#;

#[derive(Debug, Args)]
#[command(after_help = BACKUP_HELP, term_width = 0)]
pub struct BackupArgs {
    #[command(subcommand)]
    command: BackupCommands,
}

#[derive(Debug, Subcommand)]
enum BackupCommands {
    /// Exports all supported Sona application state to a backup archive.
    Export(BackupExportArgs),
    /// Inspects and validates a backup archive without changing application state.
    Inspect(BackupInspectArgs),
    /// Replaces all supported Sona application state from a backup archive.
    Import(BackupImportArgs),
}

#[derive(Debug, Args)]
struct BackupExportArgs {
    /// Existing application data directory containing the Sona databases.
    #[arg(long, value_name = "DIR")]
    app_data_dir: PathBuf,
    /// Destination backup archive. An existing regular file is replaced atomically.
    #[arg(long, value_name = "ARCHIVE")]
    output: PathBuf,
    /// Application version recorded in the backup manifest.
    #[arg(long, value_name = "VERSION")]
    app_version: String,
}

#[derive(Debug, Args)]
struct BackupInspectArgs {
    /// Existing backup archive to validate and preview.
    #[arg(long, value_name = "ARCHIVE")]
    archive: PathBuf,
}

#[derive(Debug, Args)]
struct BackupImportArgs {
    /// Existing application data directory whose backed-up state will be replaced.
    #[arg(long, value_name = "DIR")]
    app_data_dir: PathBuf,
    /// Existing backup archive to import.
    #[arg(long, value_name = "ARCHIVE")]
    archive: PathBuf,
    /// Name used when migrating legacy replacement and hotword rules.
    #[arg(long, value_name = "NAME")]
    default_rule_set_name: String,
    /// Confirms the destructive atomic replacement. No interactive prompt is opened.
    #[arg(long)]
    confirm_replace: bool,
}

enum ValidatedBackupCommand {
    Export {
        app_data_dir: PathBuf,
        request: BackupExportRequest,
    },
    Inspect(BackupInspectRequest),
    Import {
        app_data_dir: PathBuf,
        request: BackupImportRequest,
    },
}

impl ValidatedBackupCommand {
    fn app_data_dir(&self) -> PathBuf {
        match self {
            Self::Export { app_data_dir, .. } | Self::Import { app_data_dir, .. } => {
                app_data_dir.clone()
            }
            Self::Inspect(_) => PathBuf::new(),
        }
    }
}

pub fn run_backup(args: BackupArgs) -> CliResult<CliOutput> {
    let command = validate_command(args.command)?;
    let archive = FsBackupArchiveRepository::new();
    let state = LazySqliteBackupStateRepository::new(command.app_data_dir());
    let clock = SystemClock;
    let service = BackupService::new(&archive, &state, &clock);

    match command {
        ValidatedBackupCommand::Export { request, .. } => service
            .export_archive(request)
            .map_err(map_backup_error)
            .and_then(canonical_json),
        ValidatedBackupCommand::Inspect(request) => service
            .inspect_archive(request)
            .map_err(map_backup_error)
            .and_then(canonical_json),
        ValidatedBackupCommand::Import { request, .. } => service
            .import_archive(request)
            .map_err(map_backup_error)
            .and_then(canonical_json),
    }
}

fn validate_command(command: BackupCommands) -> CliResult<ValidatedBackupCommand> {
    match command {
        BackupCommands::Export(args) => validate_export(args),
        BackupCommands::Inspect(args) => validate_inspect(args),
        BackupCommands::Import(args) => validate_import(args),
    }
}

fn validate_export(args: BackupExportArgs) -> CliResult<ValidatedBackupCommand> {
    require_non_empty(&args.app_version, "app_version")?;

    let output = absolute_path(args.output, "Backup output")?;
    if output.file_name().is_none() || output.is_dir() {
        return Err(CliError::Validation(format!(
            "Backup output path must name a file: {}",
            output.display()
        )));
    }

    let app_data_dir = absolute_path(args.app_data_dir, "Application data directory")?;
    require_existing_directory(&app_data_dir)?;
    let archive_path = utf8_path(&output, "Backup output")?;
    let _ = utf8_path(&app_data_dir, "Application data directory")?;

    Ok(ValidatedBackupCommand::Export {
        app_data_dir,
        request: BackupExportRequest {
            archive_path,
            app_version: args.app_version,
        },
    })
}

fn validate_inspect(args: BackupInspectArgs) -> CliResult<ValidatedBackupCommand> {
    let archive = absolute_path(args.archive, "Backup archive")?;
    require_existing_archive(&archive)?;
    Ok(ValidatedBackupCommand::Inspect(BackupInspectRequest {
        archive_path: utf8_path(&archive, "Backup archive")?,
    }))
}

fn validate_import(args: BackupImportArgs) -> CliResult<ValidatedBackupCommand> {
    require_non_empty(&args.default_rule_set_name, "default_rule_set_name")?;

    let archive = absolute_path(args.archive, "Backup archive")?;
    let app_data_dir = absolute_path(args.app_data_dir, "Application data directory")?;
    if args.confirm_replace {
        require_existing_archive(&archive)?;
    }
    let archive_path = utf8_path(&archive, "Backup archive")?;
    let _ = utf8_path(&app_data_dir, "Application data directory")?;

    Ok(ValidatedBackupCommand::Import {
        app_data_dir,
        request: BackupImportRequest {
            archive_path,
            default_rule_set_name: args.default_rule_set_name,
            confirm_replace: args.confirm_replace,
        },
    })
}

fn require_non_empty(value: &str, field: &str) -> CliResult<()> {
    if value.trim().is_empty() {
        return Err(map_backup_error(BackupError::InvalidRequest(format!(
            "{field} is required."
        ))));
    }
    Ok(())
}

fn absolute_path(path: PathBuf, label: &str) -> CliResult<PathBuf> {
    std::path::absolute(path).map_err(|error| CliError::Io(format!("{label}: {error}")))
}

fn require_existing_archive(path: &Path) -> CliResult<()> {
    if path.is_file() {
        return Ok(());
    }
    Err(CliError::Validation(format!(
        "Backup archive does not exist or is not a file: {}",
        path.display()
    )))
}

fn require_existing_directory(path: &Path) -> CliResult<()> {
    if path.is_dir() {
        return Ok(());
    }
    Err(CliError::Validation(format!(
        "Application data directory does not exist or is not a directory: {}",
        path.display()
    )))
}

fn utf8_path(path: &Path, label: &str) -> CliResult<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| CliError::Validation(format!("{label} path must be valid UTF-8.")))
}

fn canonical_json(value: impl Serialize) -> CliResult<CliOutput> {
    let canonical =
        serde_json::to_value(value).map_err(|error| CliError::Serialize(error.to_string()))?;
    serde_json::to_string(&canonical)
        .map(CliOutput::stdout)
        .map_err(|error| CliError::Serialize(error.to_string()))
}

fn map_backup_error(error: BackupError) -> CliError {
    let message = error.to_string();
    match error {
        BackupError::InvalidRequest(_)
        | BackupError::InvalidBackup(_)
        | BackupError::ConfirmationRequired
        | BackupError::Config(_) => CliError::Validation(message),
        BackupError::Archive(_) | BackupError::State(_) => CliError::Io(message),
    }
}
