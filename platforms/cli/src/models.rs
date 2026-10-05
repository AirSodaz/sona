use clap::{Args, Subcommand};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{CliError, CliOutput, CliResult};
use sona_core::models::catalog::{ModelListEntry, ModelListFilter, ModelSummary, select_models};
use sona_core::models::downloads::{
    ResolvedModelDownload, required_companion_models, resolve_model_download,
};
use sona_model_downloads::{download_model, installed_model_is_valid};
use sona_runtime_fs::list_models as list_model_catalog;

#[derive(Debug, Args)]
#[command(
    about = "Manage, download, verify, and inspect preset ASR models",
    after_help = "Examples:\n  sona-cli models list\n  sona-cli models list --recommended\n  sona-cli models download whisper-turbo\n  sona-cli models info whisper-turbo\n  sona-cli models verify whisper-turbo\n  sona-cli models verify --all\n  sona-cli models delete whisper-turbo -y\n  sona-cli models path"
)]
pub struct ModelsArgs {
    #[command(subcommand)]
    pub command: ModelCommands,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    /// Lists preset models known to the CLI.
    #[command(
        after_help = "Examples:\n  sona-cli models list\n  sona-cli models list --mode batch --type whisper\n  sona-cli models list --language zh --installed"
    )]
    List(ModelListArgs),
    /// Downloads a preset model into the models directory.
    #[command(
        after_help = "Examples:\n  sona-cli models download sherpa-onnx-whisper-turbo\n  sona-cli models download silero-vad --models-dir ./models"
    )]
    Download(ModelDownloadArgs),
    /// Deletes an installed preset model from the models directory.
    #[command(
        after_help = "Examples:\n  sona-cli models delete sherpa-onnx-whisper-turbo --models-dir ./models --yes\n  sona-cli models delete silero-vad --models-dir ./models --yes"
    )]
    Delete(ModelDeleteArgs),
    /// Verifies the integrity of an installed preset model.
    #[command(
        after_help = "Examples:\n  sona-cli models verify whisper-turbo\n  sona-cli models verify sherpa-onnx-whisper-turbo --models-dir ./models"
    )]
    Verify(ModelVerifyArgs),
    /// Prints the resolved models directory path.
    #[command(
        after_help = "Examples:\n  sona-cli models path\n  sona-cli models path --models-dir ./models"
    )]
    Path(ModelPathArgs),
    /// Displays detailed metadata and configuration for a preset model.
    #[command(
        alias = "inspect",
        after_help = "Examples:\n  sona-cli models info whisper-turbo\n  sona-cli models info sensevoice -j\n  sona-cli models info silero-vad --models-dir ./models"
    )]
    Info(ModelInfoArgs),
}

#[derive(Debug, Args)]
#[command(about = "Display detailed metadata and configuration for a preset model")]
pub struct ModelInfoArgs {
    /// Preset model id or alias to inspect.
    #[arg(help = "Preset model id or alias, for example whisper-turbo or sensevoice")]
    pub model_id: String,
    /// Override the target models directory.
    #[arg(long, help = "Override the target models directory")]
    pub models_dir: Option<PathBuf>,
    /// Prints machine-readable JSON.
    #[arg(short = 'j', long, help = "Print machine-readable JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(about = "Print the resolved models directory path")]
pub struct ModelPathArgs {
    /// Override the models directory.
    #[arg(long, help = "Override the target models directory")]
    pub models_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[command(
    about = "Verify the integrity of installed preset models. Defaults to verifying all installed models when MODEL_ID is omitted"
)]
pub struct ModelVerifyArgs {
    /// Preset model id or alias to verify. Defaults to all installed models when omitted.
    #[arg(
        help = "Preset model id, for example whisper-turbo or silero-vad (defaults to all installed models when omitted)",
        conflicts_with = "all"
    )]
    pub model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the models directory")]
    pub models_dir: Option<PathBuf>,
    /// Verify all installed models in the models directory.
    #[arg(
        long,
        help = "Verify all installed models in the models directory",
        conflicts_with = "model_id"
    )]
    pub all: bool,
}

#[derive(Debug, Args)]
#[command(about = "List preset models with optional filters")]
pub struct ModelListArgs {
    /// Models directory containing installed presets.
    #[arg(
        long,
        help = "Override the models directory used to detect installed models"
    )]
    models_dir: Option<PathBuf>,
    /// Filter by supported mode: live or batch.
    #[arg(
        long,
        value_name = "MODE",
        value_parser = ["live", "batch"],
        help = "Filter by mode: live or batch"
    )]
    mode: Option<String>,
    #[arg(
        short = 't',
        long = "type",
        value_name = "TYPE",
        help = "Filter by type, for example whisper, vad, punctuation"
    )]
    model_type: Option<String>,
    /// Filter by language token.
    #[arg(
        short = 'l',
        long,
        value_name = "LANG",
        help = "Filter by language token, for example zh, en, ja, yue"
    )]
    language: Option<String>,
    /// Show only recommended preset models.
    #[arg(short = 'r', long, help = "Only include recommended preset models")]
    recommended: bool,
    /// Show only installed models.
    #[arg(
        short = 'i',
        long,
        help = "Only include models already present in the models directory"
    )]
    installed: bool,
    /// Include auxiliary companion models (VAD, punctuation, speaker embedding).
    #[arg(
        short = 'a',
        long = "all",
        alias = "all-types",
        help = "Include auxiliary companion models (VAD, punctuation, speaker embedding)"
    )]
    all: bool,
    /// Prints JSON instead of the default table output.
    #[arg(short = 'j', long, help = "Print machine-readable JSON")]
    json: bool,
    /// Optional search term to filter models by keyword matching ID, alias, or type.
    #[arg(
        value_name = "QUERY",
        help = "Filter models by keyword matching ID, alias, or type"
    )]
    query: Option<String>,
}

#[derive(Debug, Args)]
#[command(
    about = "Download a preset model and any required companion models",
    after_help = "Required companion models are downloaded automatically when the preset needs VAD or punctuation.\n\nExamples:\n  sona-cli models download whisper-turbo\n  sona-cli models download sensevoice\n  sona-cli models download silero-vad --models-dir ./models"
)]
pub struct ModelDownloadArgs {
    /// Preset model id(s) or alias(es) to download.
    #[arg(
        help = "Preset model id(s) or alias(es), for example whisper-turbo or sensevoice",
        required = true,
        num_args = 1..
    )]
    model_ids: Vec<String>,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the target models directory")]
    models_dir: Option<PathBuf>,
    /// Suppresses progress logs.
    #[arg(short = 'q', long, help = "Hide per-download progress output")]
    quiet: bool,
    /// Download mirror strategy: auto, direct, ghproxy, ghnet, or hf-mirror.
    #[arg(
        long,
        value_name = "MIRROR",
        value_parser = ["auto", "direct", "ghproxy", "ghnet", "hf-mirror"],
        help = "Download mirror strategy: auto, direct, ghproxy, ghnet, or hf-mirror"
    )]
    pub mirror: Option<String>,
    /// Overwrites invalid installed files without prompting.
    #[arg(
        short = 'y',
        long,
        help = "Overwrite invalid files without prompting for confirmation"
    )]
    yes: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Delete one or more installed preset models",
    after_help = "Companion models are not deleted automatically. Pass --yes to confirm deletion without prompting.\n\nExamples:\n  sona-cli models delete sherpa-onnx-whisper-turbo --yes\n  sona-cli models delete whisper-turbo silero-vad -y\n  sona-cli models delete --all -y\n  sona-cli models delete silero-vad --models-dir ./models --yes"
)]
pub struct ModelDeleteArgs {
    /// Preset model id(s) or alias(es) to delete.
    #[arg(
        value_name = "MODEL_ID",
        help = "Preset model id(s) or alias(es) to delete, for example sherpa-onnx-whisper-turbo or silero-vad",
        conflicts_with = "all",
        required_unless_present = "all",
        num_args = 1..
    )]
    model_ids: Vec<String>,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the models directory")]
    models_dir: Option<PathBuf>,
    /// Confirms deletion without an interactive prompt.
    #[arg(short = 'y', long, help = "Delete without prompting for confirmation")]
    yes: bool,
    /// Delete all installed preset models.
    #[arg(
        long,
        help = "Delete all installed preset models in the models directory",
        conflicts_with = "model_ids"
    )]
    all: bool,
}

pub async fn run_models(
    args: ModelsArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    match args.command {
        ModelCommands::List(args) => run_model_list(args),
        ModelCommands::Download(args) => run_model_download(args, io).await,
        ModelCommands::Delete(args) => run_model_delete(args, io),
        ModelCommands::Verify(args) => run_model_verify(args).await,
        ModelCommands::Path(args) => run_model_path(args),
        ModelCommands::Info(args) => run_model_info(args).await,
    }
}

fn run_model_path(args: ModelPathArgs) -> CliResult<CliOutput> {
    let models_dir = resolve_models_dir(args.models_dir)?;
    Ok(CliOutput::stdout(models_dir.display().to_string()))
}

async fn run_model_verify(args: ModelVerifyArgs) -> CliResult<CliOutput> {
    let models_dir = resolve_models_dir(args.models_dir)?;
    if args.all || args.model_id.is_none() {
        let installed = list_models(Some(models_dir.clone()))?
            .into_iter()
            .filter(|m| m.installed)
            .collect::<Vec<_>>();
        if installed.is_empty() {
            return Ok(CliOutput::stdout(format!(
                "No installed models found in {}",
                models_dir.display()
            )));
        }
        let mut results = Vec::new();
        let mut failed = 0;
        for model in &installed {
            let resolved = resolve_model_download(&model.id, &models_dir)
                .map_err(|error| CliError::Validation(error.to_string()))?;
            let is_valid = installed_model_is_valid(&resolved)
                .await
                .map_err(map_download_error)?;
            if is_valid {
                results.push(format!("  [OK]   {}", model.id));
            } else {
                results.push(format!(
                    "  [FAIL] {} (corrupted or incomplete files)",
                    model.id
                ));
                failed += 1;
            }
        }
        let summary = format!(
            "Verified {} installed model(s) in {}:\n{}\nTotal: {} valid, {} corrupted.",
            installed.len(),
            models_dir.display(),
            results.join("\n"),
            installed.len() - failed,
            failed
        );
        if failed > 0 {
            return Err(CliError::Model(summary));
        } else {
            return Ok(CliOutput::stdout(summary));
        }
    }

    let model_id = args.model_id.as_ref().unwrap();
    let resolved = resolve_model_download(model_id, &models_dir)
        .map_err(|error| CliError::Validation(error.to_string()))?;
    if !sona_runtime_fs::path_exists(&resolved.install_path)
        .map_err(|error| CliError::Io(error.to_string()))?
    {
        return Err(CliError::Model(format!(
            "Model '{}' is not installed at {}",
            resolved.model.id,
            resolved.install_path.display()
        )));
    }

    let is_valid = installed_model_is_valid(&resolved)
        .await
        .map_err(map_download_error)?;

    if is_valid {
        Ok(CliOutput::stdout(format!(
            "Model '{}' at {} is valid and intact.",
            resolved.model.id,
            resolved.install_path.display()
        )))
    } else {
        Err(CliError::Model(format!(
            "Model '{}' at {} failed verification (corrupted or incomplete files). Run 'sona-cli models download {}' to repair.",
            resolved.model.id,
            resolved.install_path.display(),
            model_id
        )))
    }
}

fn run_model_list(args: ModelListArgs) -> CliResult<CliOutput> {
    let mut models = select_models(
        list_models(args.models_dir.clone())?,
        &ModelListFilter {
            mode: args.mode.clone(),
            model_type: args.model_type.clone(),
            language: args.language.clone(),
            installed_only: args.installed,
        },
    );
    if let Some(query) = &args.query {
        let q = query.trim().to_ascii_lowercase();
        if !q.is_empty() {
            models.retain(|m| {
                m.id.to_ascii_lowercase().contains(&q)
                    || m.name.to_ascii_lowercase().contains(&q)
                    || m.model_type.to_ascii_lowercase().contains(&q)
                    || sona_core::models::preset_models::aliases_for_preset_model(&m.id)
                        .iter()
                        .any(|a| a.to_ascii_lowercase().contains(&q))
            });
        }
    }
    if args.recommended {
        models.retain(|m| {
            sona_core::models::preset_models::find_preset_model(&m.id)
                .and_then(|p| p.is_recommended)
                .unwrap_or(false)
        });
    }
    let has_explicit_query = args.query.as_ref().is_some_and(|q| !q.trim().is_empty());
    if !args.all && args.model_type.is_none() && !has_explicit_query {
        models.retain(|m| {
            m.model_type != "vad"
                && m.model_type != "punctuation"
                && !m.model_type.starts_with("speaker-")
        });
    }
    let output = if args.json {
        serde_json::to_string_pretty(
            &models
                .into_iter()
                .map(ModelListEntry::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| CliError::Serialize(format!("Failed to serialize model list: {error}")))?
    } else {
        render_model_table(&models)
    };

    Ok(CliOutput::stdout(output))
}

async fn run_model_download(
    args: ModelDownloadArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    let quiet = args.quiet;
    let yes = args.yes;
    let mirror = args
        .mirror
        .as_deref()
        .map(sona_model_downloads::parse_download_mirror);
    let models_dir = resolve_models_dir(args.models_dir)?;
    let mut stderr_lines = Vec::new();

    let mut download_queue: Vec<ResolvedModelDownload> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for model_id in &args.model_ids {
        let resolved = resolve_model_download(model_id, &models_dir)
            .map_err(|error| CliError::Validation(error.to_string()))?;
        if seen_ids.insert(resolved.model.id.clone()) {
            let companions = required_companion_models(&resolved.model);
            let mut resolved_companions = Vec::new();
            for companion_id in companions.companion_model_ids() {
                let companion = resolve_model_download(&companion_id, &models_dir)
                    .map_err(|error| CliError::Validation(error.to_string()))?;
                resolved_companions.push(companion);
            }
            download_queue.push(resolved);
            for companion in resolved_companions {
                if seen_ids.insert(companion.model.id.clone()) {
                    download_queue.push(companion);
                }
            }
        }
    }

    for model in &download_queue {
        download_one_model(model, yes, quiet, mirror, &mut stderr_lines, io).await?;
    }
    Ok(CliOutput::stderr(stderr_lines.join("\n")))
}

fn run_model_delete(
    args: ModelDeleteArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    let models_dir = resolve_models_dir(args.models_dir)?;

    if args.all {
        let installed = list_models(Some(models_dir.clone()))?
            .into_iter()
            .filter(|m| m.installed)
            .collect::<Vec<_>>();
        if installed.is_empty() {
            return Ok(CliOutput::stderr(format!(
                "No installed models found in {}",
                models_dir.display()
            )));
        }

        if !args.yes {
            if !io.stdin_is_terminal() {
                return Err(CliError::Validation(
                    "Cannot prompt for confirmation in non-interactive shell. Pass --yes to confirm deletion."
                        .to_string(),
                ));
            }

            write!(
                io.stderr(),
                "Are you sure you want to delete all {} installed model(s) in {}? [y/N] ",
                installed.len(),
                models_dir.display()
            )
            .map_err(|error| {
                CliError::Io(format!("Failed to write confirmation prompt: {error}"))
            })?;
            io.stderr().flush().map_err(|error| {
                CliError::Io(format!("Failed to flush confirmation prompt: {error}"))
            })?;

            let mut answer = String::new();
            io.read_line_stdin(&mut answer)
                .map_err(|error| CliError::Io(format!("Failed to read confirmation: {error}")))?;
            if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                return Ok(CliOutput::stderr("Deletion cancelled.".to_string()));
            }
        }

        sona_llama_cpp::prune_idle_llm_models();
        sona_llama_cpp::prune_idle_llama_models();

        let mut deleted = Vec::new();
        let mut failed = Vec::new();
        for model in &installed {
            match sona_model_downloads::delete_installed_model(&models_dir, &model.id) {
                Ok(sona_model_downloads::DeleteModelResult::Deleted(path)) => {
                    deleted.push((model.id.clone(), path));
                }
                Ok(sona_model_downloads::DeleteModelResult::NotInstalled(_)) => {}
                Err(error) => {
                    failed.push((model.id.clone(), error));
                }
            }
        }

        if !failed.is_empty() {
            let error_msgs = failed
                .iter()
                .map(|(id, err)| format!("{id}: {err}"))
                .collect::<Vec<_>>()
                .join("; ");
            if !deleted.is_empty() {
                return Err(CliError::Model(format!(
                    "Deleted {} model(s), but {} model(s) failed to delete: {}",
                    deleted.len(),
                    failed.len(),
                    error_msgs
                )));
            } else {
                return Err(CliError::Model(format!(
                    "Failed to delete {} model(s): {}",
                    failed.len(),
                    error_msgs
                )));
            }
        }

        let deleted_lines = deleted
            .iter()
            .map(|(id, path)| format!("  - {id} ({})", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(CliOutput::stderr(format!(
            "Deleted {} installed model(s) from {}:\n{}",
            deleted.len(),
            models_dir.display(),
            deleted_lines
        )));
    }

    if args.model_ids.is_empty() {
        return Err(CliError::Validation(
            "Specify one or more model ids or aliases to delete, or pass --all to delete all installed models."
                .to_string(),
        ));
    }

    let mut resolved_models = Vec::new();
    for mid in &args.model_ids {
        let resolved = resolve_model_download(mid, &models_dir)
            .map_err(|error| CliError::Validation(error.to_string()))?;
        resolved_models.push((mid.clone(), resolved));
    }

    let (installed_targets, uninstalled_targets): (Vec<_>, Vec<_>) = resolved_models
        .into_iter()
        .partition(|(_, resolved)| resolved.install_path.exists());

    if installed_targets.is_empty() {
        if args.model_ids.len() == 1 {
            let mid = &args.model_ids[0];
            let resolved = resolve_model_download(mid, &models_dir)
                .map_err(|error| CliError::Validation(error.to_string()))?;
            return Ok(CliOutput::stderr(format!(
                "Model {} is not installed at {}",
                mid,
                resolved.install_path.display()
            )));
        } else {
            return Ok(CliOutput::stderr(
                "None of the specified models are installed.".to_string(),
            ));
        }
    }

    if !args.yes {
        if !io.stdin_is_terminal() {
            return Err(CliError::Validation(
                "Cannot prompt for confirmation in non-interactive shell. Pass --yes to confirm deletion."
                    .to_string(),
            ));
        }

        if installed_targets.len() == 1 {
            write!(
                io.stderr(),
                "Are you sure you want to delete model {}? [y/N] ",
                installed_targets[0].0
            )
            .map_err(|error| {
                CliError::Io(format!("Failed to write confirmation prompt: {error}"))
            })?;
        } else {
            let names = installed_targets
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                io.stderr(),
                "Are you sure you want to delete {} model(s) ({})? [y/N] ",
                installed_targets.len(),
                names
            )
            .map_err(|error| {
                CliError::Io(format!("Failed to write confirmation prompt: {error}"))
            })?;
        }
        io.stderr().flush().map_err(|error| {
            CliError::Io(format!("Failed to flush confirmation prompt: {error}"))
        })?;

        let mut answer = String::new();
        io.read_line_stdin(&mut answer)
            .map_err(|error| CliError::Io(format!("Failed to read confirmation: {error}")))?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Ok(CliOutput::stderr("Deletion cancelled.".to_string()));
        }
    }

    sona_llama_cpp::prune_idle_llm_models();
    sona_llama_cpp::prune_idle_llama_models();

    let uninstalled_note = if !uninstalled_targets.is_empty() && args.model_ids.len() > 1 {
        let uninstalled_names = uninstalled_targets
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "\nNote: {} model(s) not installed, skipped: {}",
            uninstalled_targets.len(),
            uninstalled_names
        )
    } else {
        String::new()
    };

    if installed_targets.len() == 1 {
        let (mid, _) = &installed_targets[0];
        match sona_model_downloads::delete_installed_model(&models_dir, mid) {
            Ok(sona_model_downloads::DeleteModelResult::Deleted(path)) => {
                Ok(CliOutput::stderr(format!(
                    "Deleted {} from {}{}",
                    mid,
                    path.display(),
                    uninstalled_note
                )))
            }
            Ok(sona_model_downloads::DeleteModelResult::NotInstalled(path)) => {
                Ok(CliOutput::stderr(format!(
                    "Model {} is not installed at {}{}",
                    mid,
                    path.display(),
                    uninstalled_note
                )))
            }
            Err(error) => Err(map_download_error(error)),
        }
    } else {
        let mut deleted = Vec::new();
        let mut failed = Vec::new();
        for (mid, _) in &installed_targets {
            match sona_model_downloads::delete_installed_model(&models_dir, mid) {
                Ok(sona_model_downloads::DeleteModelResult::Deleted(path)) => {
                    deleted.push((mid.clone(), path));
                }
                Ok(sona_model_downloads::DeleteModelResult::NotInstalled(_)) => {}
                Err(error) => {
                    failed.push((mid.clone(), error));
                }
            }
        }

        if !failed.is_empty() {
            let error_msgs = failed
                .iter()
                .map(|(id, err)| format!("{id}: {err}"))
                .collect::<Vec<_>>()
                .join("; ");
            if !deleted.is_empty() {
                return Err(CliError::Model(format!(
                    "Deleted {} model(s), but {} model(s) failed to delete: {}",
                    deleted.len(),
                    failed.len(),
                    error_msgs
                )));
            } else {
                return Err(CliError::Model(format!(
                    "Failed to delete {} model(s): {}",
                    failed.len(),
                    error_msgs
                )));
            }
        }

        let deleted_lines = deleted
            .iter()
            .map(|(id, path)| format!("  - {id} ({})", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(CliOutput::stderr(format!(
            "Deleted {} installed model(s) from {}:\n{}{}",
            deleted.len(),
            models_dir.display(),
            deleted_lines,
            uninstalled_note
        )))
    }
}

#[derive(serde::Serialize)]
struct ModelInfoJson {
    id: String,
    name: String,
    aliases: Vec<String>,
    #[serde(rename = "type")]
    model_type: String,
    modes: Vec<String>,
    size: String,
    installed: bool,
    installed_valid: bool,
    install_path: String,
    languages: Vec<String>,
    language_mode: String,
    companion_models: Vec<String>,
    artifacts: Vec<ModelInfoArtifactJson>,
}

#[derive(serde::Serialize)]
struct ModelInfoArtifactJson {
    filename: String,
    url: String,
    sha256: Option<String>,
    size_bytes: Option<u64>,
}

async fn run_model_info(args: ModelInfoArgs) -> CliResult<CliOutput> {
    let models_dir = resolve_models_dir(args.models_dir)?;
    let resolved = resolve_model_download(&args.model_id, &models_dir)
        .map_err(|error| CliError::Validation(error.to_string()))?;

    let is_installed = resolved.install_path.exists();
    let is_valid = if is_installed {
        installed_model_is_valid(&resolved).await.unwrap_or(false)
    } else {
        false
    };

    let aliases = sona_core::models::preset_models::aliases_for_preset_model(&resolved.model.id);
    let companions = required_companion_models(&resolved.model);
    let companion_ids = companions
        .companion_model_ids()
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let language_mode_str = match resolved.model.language_mode {
        sona_core::models::preset_models::LanguageMode::Selectable => "selectable",
        sona_core::models::preset_models::LanguageMode::Auto => "auto",
        sona_core::models::preset_models::LanguageMode::Fixed => "fixed",
        sona_core::models::preset_models::LanguageMode::None => "none",
    };

    if args.json {
        let json_obj = ModelInfoJson {
            id: resolved.model.id.clone(),
            name: resolved.model.name.clone(),
            aliases,
            model_type: resolved.model.model_type.clone(),
            modes: resolved.model.modes.clone().unwrap_or_default(),
            size: resolved.model.size.clone(),
            installed: is_installed,
            installed_valid: is_valid,
            install_path: resolved.install_path.display().to_string(),
            languages: resolved.model.languages.clone(),
            language_mode: language_mode_str.to_string(),
            companion_models: companion_ids,
            artifacts: resolved
                .model
                .artifacts
                .iter()
                .map(|a| ModelInfoArtifactJson {
                    filename: a.filename.clone(),
                    url: a.url.clone(),
                    sha256: a.sha256.clone(),
                    size_bytes: a.size_bytes,
                })
                .collect(),
        };
        let output = serde_json::to_string_pretty(&json_obj)
            .map_err(|e| CliError::Serialize(e.to_string()))?;
        return Ok(CliOutput::stdout(output));
    }

    let install_status = if is_installed {
        if is_valid {
            format!("Yes (valid, at {})", resolved.install_path.display())
        } else {
            format!(
                "Yes (corrupted/incomplete, at {})",
                resolved.install_path.display()
            )
        }
    } else {
        format!("No (expected at {})", resolved.install_path.display())
    };

    let alias_str = if aliases.is_empty() {
        "-".to_string()
    } else {
        aliases.join(", ")
    };

    let modes_str = resolved
        .model
        .modes
        .as_ref()
        .map(|m| m.join(", "))
        .unwrap_or_else(|| "-".to_string());

    let companions_str = if companion_ids.is_empty() {
        "None".to_string()
    } else {
        companion_ids.join(", ")
    };

    let languages_str = if resolved.model.languages.is_empty() {
        "-".to_string()
    } else {
        format!(
            "{} ({language_mode_str})",
            resolved.model.languages.join(", ")
        )
    };

    let mut lines = Vec::new();
    lines.push("Model Information:".to_string());
    lines.push(format!("  ID:          {}", resolved.model.id));
    lines.push(format!("  Name:        {}", resolved.model.name));
    lines.push(format!("  Aliases:     {}", alias_str));
    lines.push(format!("  Type:        {}", resolved.model.model_type));
    lines.push(format!("  Modes:       {}", modes_str));
    lines.push(format!("  Size:        {}", resolved.model.size));
    lines.push(format!("  Installed:   {}", install_status));
    lines.push(format!("  Languages:   {}", languages_str));
    lines.push(format!("  Companions:  {}", companions_str));
    if !resolved.model.artifacts.is_empty() {
        lines.push("  Artifacts:".to_string());
        for artifact in &resolved.model.artifacts {
            let sha = artifact
                .sha256
                .as_deref()
                .map(|s| {
                    if s.len() > 16 {
                        format!("sha256: {}...", &s[..16])
                    } else {
                        format!("sha256: {s}")
                    }
                })
                .unwrap_or_else(|| "no hash".to_string());
            let size = artifact
                .size_bytes
                .map(|b| {
                    if b >= 1024 * 1024 * 1024 {
                        format!("{:.2} GB", b as f64 / (1024.0 * 1024.0 * 1024.0))
                    } else if b >= 1024 * 1024 {
                        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
                    } else if b >= 1024 {
                        format!("{:.1} KB", b as f64 / 1024.0)
                    } else {
                        format!("{b} B")
                    }
                })
                .unwrap_or_else(|| "variable".to_string());
            lines.push(format!("    - {} ({}, {})", artifact.filename, size, sha));
        }
    }

    Ok(CliOutput::stdout(lines.join("\n")))
}

async fn download_model_with_mirror_choice<F>(
    resolved: &ResolvedModelDownload,
    mirror: Option<sona_model_downloads::DownloadMirror>,
    on_progress: F,
) -> Result<PathBuf, sona_model_downloads::DownloadError>
where
    F: FnMut(u64, u64) + Send + 'static,
{
    match mirror {
        None | Some(sona_model_downloads::DownloadMirror::Auto) => {
            download_model(resolved, on_progress).await
        }
        Some(strategy) => {
            let notify = std::sync::Arc::new(tokio::sync::Notify::new());
            let notify_clone = notify.clone();
            let ctrl_c_task = tokio::spawn(async move {
                if let Ok(()) = tokio::signal::ctrl_c().await {
                    notify_clone.notify_one();
                }
            });

            let mut on_progress = on_progress;
            let result = sona_model_downloads::download_model_with_cancel_and_mirror(
                resolved,
                notify,
                strategy,
                move |progress| {
                    if progress.stage == sona_model_downloads::ModelDownloadStage::Downloading {
                        on_progress(progress.downloaded_bytes, progress.total_bytes);
                    }
                },
            )
            .await;

            ctrl_c_task.abort();
            result
        }
    }
}

async fn download_one_model(
    resolved: &ResolvedModelDownload,
    yes: bool,
    quiet: bool,
    mirror: Option<sona_model_downloads::DownloadMirror>,
    stderr_lines: &mut Vec<String>,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<()> {
    if installed_model_is_valid(resolved)
        .await
        .map_err(map_download_error)?
    {
        stderr_lines.push(format!(
            "Installed {} at {}",
            resolved.model.id,
            resolved.install_path.display()
        ));
        return Ok(());
    }

    let install_path_exists = sona_runtime_fs::path_exists(&resolved.install_path)
        .map_err(|error| CliError::Io(error.to_string()))?;
    if install_path_exists
        && !yes
        && !confirm_model_overwrite(&resolved.model.id, &resolved.install_path, io)?
    {
        return Err(CliError::Model(
            "Download cancelled: model files are invalid and user declined to overwrite."
                .to_string(),
        ));
    }

    let stderr_is_terminal = io.stderr_is_terminal();
    let has_printed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let has_printed_clone = has_printed.clone();
    let display_id = resolved.model.id.clone();
    let mut last_percentage: Option<i32> = None;

    let install_path =
        download_model_with_mirror_choice(resolved, mirror, move |downloaded, total| {
            if quiet || total == 0 {
                return;
            }
            let percentage = ((downloaded as f64 / total as f64) * 100.0).round() as i32;
            if stderr_is_terminal {
                eprint!("\rDownloading {display_id}: {percentage}%");
                let _ = io::stderr().flush();
                has_printed_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            } else if (percentage == 100 || percentage % 10 == 0)
                && last_percentage != Some(percentage)
            {
                eprintln!("Downloading {display_id}: {percentage}%");
                last_percentage = Some(percentage);
            }
        })
        .await
        .map_err(map_download_error)?;

    if has_printed.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!();
    }
    stderr_lines.push(format!(
        "Installed {} at {}",
        resolved.model.id,
        install_path.display()
    ));
    Ok(())
}

fn confirm_model_overwrite(
    model_id: &str,
    install_path: &Path,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<bool> {
    if !io.stdin_is_terminal() {
        return Err(CliError::Validation(
            "Cannot prompt for confirmation in non-interactive shell. Use --yes to override."
                .to_string(),
        ));
    }

    write!(
        io.stderr(),
        "Model {model_id} already exists at {} but is invalid (checksum mismatch). Overwrite? [y/N] ",
        install_path.display()
    )
    .map_err(|error| CliError::Io(format!("Failed to write confirmation prompt: {error}")))?;
    io.stderr()
        .flush()
        .map_err(|error| CliError::Io(format!("Failed to flush confirmation prompt: {error}")))?;

    let mut answer = String::new();
    io.read_line_stdin(&mut answer)
        .map_err(|error| CliError::Io(format!("Failed to read confirmation: {error}")))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn list_models(models_dir: Option<PathBuf>) -> CliResult<Vec<ModelSummary>> {
    let models_dir = resolve_models_dir(models_dir)?;
    Ok(list_model_catalog(&models_dir))
}

fn resolve_models_dir(configured: Option<PathBuf>) -> CliResult<PathBuf> {
    sona_core::models::paths::resolve_models_dir(
        configured,
        crate::desktop_paths::default_models_dir(),
        crate::desktop_paths::models_dir_status,
    )
    .map_err(|error| CliError::Validation(error.to_string()))
}

fn map_download_error(error: sona_model_downloads::DownloadError) -> CliError {
    let message = error.to_string();
    match error {
        sona_model_downloads::DownloadError::Cancelled => CliError::Cancelled(message),
        sona_model_downloads::DownloadError::Network(_)
        | sona_model_downloads::DownloadError::HttpStatus(_)
        | sona_model_downloads::DownloadError::HttpClient { .. }
        | sona_model_downloads::DownloadError::RangeNotSatisfiable => CliError::Network(message),
        sona_model_downloads::DownloadError::Io(_)
        | sona_model_downloads::DownloadError::FileSystem(_) => CliError::Io(message),
        sona_model_downloads::DownloadError::HashMismatch { .. } => CliError::Model(message),
        sona_model_downloads::DownloadError::AlreadyInProgress => CliError::Other(message),
        sona_model_downloads::DownloadError::Validation(_) => CliError::Validation(message),
    }
}

/// Terminal-friendly language column: full lists would blow up the table for
/// 100-language models, so show up to four codes plus a `+N` tail.
fn render_language_column(languages: &[String]) -> String {
    const MAX_VISIBLE: usize = 4;
    if languages.is_empty() {
        return "-".to_string();
    }
    let visible = languages.len().min(MAX_VISIBLE);
    let mut label = languages[..visible].join(",");
    let rest = languages.len() - visible;
    if rest > 0 {
        label.push_str(&format!("+{rest}"));
    }
    label
}

fn primary_alias_for_model(model_id: &str) -> Option<&'static str> {
    sona_core::models::preset_models::PRESET_MODEL_ALIASES
        .iter()
        .find(|(_, canonical)| *canonical == model_id)
        .map(|(alias, _)| *alias)
}

fn render_model_table(models: &[ModelSummary]) -> String {
    let rows = models
        .iter()
        .map(|model| {
            [
                model.id.clone(),
                primary_alias_for_model(&model.id)
                    .unwrap_or("-")
                    .to_string(),
                model.model_type.clone(),
                render_language_column(&model.languages),
                model.size.clone(),
                if model.installed { "yes" } else { "no" }.to_string(),
                model
                    .modes
                    .iter()
                    .map(|m| match m.as_str() {
                        "streaming" => "live",
                        other => other,
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            ]
        })
        .collect::<Vec<_>>();
    let headers = [
        "ID",
        "Alias",
        "Type",
        "Language",
        "Size",
        "Installed",
        "Modes",
    ];
    let mut widths = headers.map(str::len);

    for row in &rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in rows {
        let refs = [
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
            row[4].as_str(),
            row[5].as_str(),
            row[6].as_str(),
        ];
        append_table_row(&mut output, &refs, &widths);
    }
    output
}

fn append_table_row(output: &mut String, values: &[&str; 7], widths: &[usize; 7]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&format!("{value:<width$}", width = widths[index]));
    }
    output.push('\n');
}

fn append_table_separator(output: &mut String, widths: &[usize; 7]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_summary(id: &str, languages: &[&str], installed: bool) -> ModelSummary {
        ModelSummary {
            id: id.to_string(),
            name: format!("{id} name"),
            model_type: "whisper".to_string(),
            languages: languages.iter().map(|code| code.to_string()).collect(),
            language_mode: sona_core::models::preset_models::LanguageMode::Selectable,
            size: "1 MB".to_string(),
            modes: vec!["batch".to_string()],
            installed,
            install_path: PathBuf::from(format!("C:/models/{id}")),
        }
    }

    #[test]
    fn renders_model_list_as_table_with_headers() {
        let table = render_model_table(&[
            model_summary("short", &["en"], true),
            model_summary(
                "longer-model-id",
                &["af", "am", "ar", "as", "az", "ba", "zh"],
                false,
            ),
        ]);

        assert!(table.contains("ID"));
        assert!(table.contains("Alias"));
        assert!(table.contains("Type"));
        assert!(table.contains("Language"));
        assert!(table.contains("Size"));
        assert!(table.contains("Installed"));
        assert!(table.contains("Modes"));
        assert!(table.contains("longer-model-id"));
        assert!(table.contains("yes"));
        assert!(table.contains("no"));
        assert!(!table.contains("install_path"));
        // 7 languages collapse to four visible codes plus an overflow tail.
        assert!(table.contains("af,am,ar,as+3"));
    }

    #[test]
    fn language_column_collapses_long_lists_and_marks_empty() {
        assert_eq!(render_language_column(&[]), "-");
        assert_eq!(
            render_language_column(&["en".to_string(), "zh".to_string()]),
            "en,zh"
        );
        let many: Vec<String> = (0..6).map(|index| format!("l{index}")).collect();
        assert_eq!(render_language_column(&many), "l0,l1,l2,l3+2");
    }

    #[test]
    fn download_error_variants_preserve_cli_categories_and_exit_codes() {
        let network = map_download_error(sona_model_downloads::DownloadError::HttpClient {
            reason: "client unavailable".to_string(),
        });
        let filesystem = map_download_error(sona_model_downloads::DownloadError::file_system(
            sona_model_downloads::DownloadFileOperation::InspectInstall,
            "C:/models/test",
            "access denied",
        ));
        let hash = map_download_error(sona_model_downloads::DownloadError::HashMismatch {
            path: PathBuf::from("C:/models/test.download"),
            expected: "expected".to_string(),
            actual: "actual".to_string(),
        });
        let cancelled = map_download_error(sona_model_downloads::DownloadError::Cancelled);

        assert!(matches!(network, CliError::Network(_)));
        assert_eq!(network.exit_code(), 4);
        assert!(matches!(filesystem, CliError::Io(_)));
        assert_eq!(filesystem.exit_code(), 5);
        assert!(matches!(hash, CliError::Model(_)));
        assert_eq!(hash.exit_code(), 3);
        assert!(matches!(cancelled, CliError::Cancelled(_)));
        assert_eq!(cancelled.exit_code(), 130);
    }
}
