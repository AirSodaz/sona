use clap::{Args, ValueEnum};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;

use sona_core::runtime::app_update::{DEFAULT_UPDATER_PUBKEY, UpdateChannel};
use sona_model_downloads::{
    DownloadClient, DownloadError, DownloadMirror, check_app_update, check_app_update_with_version,
    download_update_package, install_update_package, parse_download_mirror,
};

use crate::{CliError, CliIo, CliOutput, CliResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum UpdateChannelArg {
    Stable,
    Nightly,
}

impl From<UpdateChannelArg> for UpdateChannel {
    fn from(arg: UpdateChannelArg) -> Self {
        match arg {
            UpdateChannelArg::Stable => UpdateChannel::Stable,
            UpdateChannelArg::Nightly => UpdateChannel::Nightly,
        }
    }
}

#[derive(Debug, Clone, Args)]
#[command(
    about = "Check for updates and download Sona Desktop installation packages (CLI self-update is not packaged; update CLI via cargo or package manager)",
    after_help = "Note: Sona release updater manifests package Sona Desktop installers (.exe, .AppImage, .tar.gz).\nTo update the sona-cli binary itself, rebuild from source via 'cargo install --path platforms/cli' or use your package manager.\n\nExamples:\n  sona-cli update\n  sona-cli update --check\n  sona-cli update --check --json\n  sona-cli update --channel nightly\n  sona-cli update --download-only -o ./setup.exe\n  sona-cli update --force -y"
)]
pub struct UpdateArgs {
    /// Only check for updates without downloading or installing.
    #[arg(long, help = "Only check for updates without installing")]
    pub check: bool,

    /// Force re-download and re-installation even if already up to date.
    #[arg(short = 'f', long, help = "Force reinstall even if already up to date")]
    pub force: bool,

    /// Update channel: stable or nightly.
    #[arg(
        long,
        default_value = "stable",
        value_name = "CHANNEL",
        help = "Update channel: stable or nightly"
    )]
    pub channel: UpdateChannelArg,

    /// Download and verify the update package without installing.
    #[arg(
        long,
        help = "Download and verify the update package without installing"
    )]
    pub download_only: bool,

    /// Target output path for downloaded package (used with --download-only).
    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        help = "Target output path for downloaded package"
    )]
    pub output: Option<PathBuf>,

    /// Download mirror strategy: auto, direct, ghproxy, or ghnet.
    #[arg(
        long,
        value_name = "STRATEGY",
        help = "Download mirror strategy: auto, direct, ghproxy, or ghnet"
    )]
    pub mirror: Option<String>,

    /// Skip confirmation prompt in interactive sessions.
    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,

    /// Suppress progress output during download.
    #[arg(short = 'q', long, help = "Hide per-download progress output")]
    pub quiet: bool,

    /// Print machine-readable JSON.
    #[arg(short = 'j', long, help = "Print machine-readable JSON")]
    pub json: bool,

    /// Custom updater.json endpoint URL (used for testing or custom distribution mirrors).
    #[arg(long, hide = true)]
    pub endpoint: Option<String>,

    /// Override current version for testing purposes.
    #[arg(long, hide = true)]
    pub current_version_override: Option<String>,
    /// Optional Minisign public key override (used for testing).
    #[arg(long, hide = true)]
    pub pubkey: Option<String>,
}

fn map_download_error(error: DownloadError) -> CliError {
    let message = error.to_string();
    match error {
        DownloadError::Cancelled => CliError::Cancelled(message),
        DownloadError::Network(_)
        | DownloadError::HttpStatus(_)
        | DownloadError::HttpClient { .. }
        | DownloadError::RangeNotSatisfiable => CliError::Network(message),
        DownloadError::Io(_) | DownloadError::FileSystem(_) => CliError::Io(message),
        DownloadError::HashMismatch { .. } => CliError::Validation(message),
        DownloadError::Validation(_) => CliError::Validation(message),
        DownloadError::AlreadyInProgress => CliError::Other(message),
    }
}

fn filename_from_url(url: &str) -> String {
    let clean = url.split(['?', '#']).next().unwrap_or(url);
    if let Some(name) = clean.rsplit('/').next()
        && !name.is_empty()
    {
        return name.to_string();
    }
    match std::env::consts::OS {
        "windows" => "sona-setup.exe".to_string(),
        "macos" => "Sona.app.tar.gz".to_string(),
        _ => "Sona.AppImage".to_string(),
    }
}

pub async fn run_update(args: UpdateArgs, io: &mut (dyn CliIo + Send)) -> CliResult<CliOutput> {
    let client = DownloadClient::try_new().map_err(map_download_error)?;
    let mirror = args
        .mirror
        .as_deref()
        .map(parse_download_mirror)
        .unwrap_or(DownloadMirror::Auto);
    let channel: UpdateChannel = args.channel.into();

    let check_result = if let Some(v_override) = &args.current_version_override {
        check_app_update_with_version(
            &client,
            args.endpoint.as_deref(),
            channel,
            mirror,
            v_override,
        )
        .await
        .map_err(map_download_error)?
    } else {
        check_app_update(&client, args.endpoint.as_deref(), channel, mirror)
            .await
            .map_err(map_download_error)?
    };

    // Mode: --check
    if args.check {
        if args.json {
            let json = serde_json::to_string_pretty(&check_result)
                .map_err(|e| CliError::Serialize(e.to_string()))?;
            return Ok(CliOutput::stdout(json));
        }

        let mut out = String::new();
        if check_result.update_available {
            out.push_str("An update is available! (Sona Desktop)\n");
            out.push_str(&format!(
                "  Current version: {}\n",
                check_result.current_version
            ));
            out.push_str(&format!(
                "  Latest version:  {} (channel: {})\n",
                check_result.latest_version, check_result.channel
            ));
            if let Some(pub_date) = &check_result.pub_date {
                out.push_str(&format!("  Release date:    {pub_date}\n"));
            }
            if let Some(notes) = &check_result.notes {
                out.push_str("  Release notes:\n");
                for line in notes.lines() {
                    out.push_str(&format!("    {line}\n"));
                }
            }
            out.push_str(&format!("  Asset URL:       {}\n", check_result.asset_url));
            out.push_str(&format!(
                "  Release URL:     {}\n\n",
                check_result.release_url
            ));
            out.push_str("  (Note: CLI self-update is not packaged in updater.json; update sona-cli via cargo or package manager)\n");
            out.push_str("Run 'sona-cli update' to download and run the Desktop installer.\n");
        } else {
            out.push_str(&format!(
                "Sona is already up to date (v{}, channel: {}).\n",
                check_result.current_version, check_result.channel
            ));
            if !args.force {
                out.push_str("Run 'sona-cli update --force' to reinstall.\n");
            }
        }
        return Ok(CliOutput::stdout(out));
    }

    // Check if an update is required or forced
    if !check_result.update_available && !args.force {
        if args.json {
            let json = serde_json::to_string_pretty(&check_result)
                .map_err(|e| CliError::Serialize(e.to_string()))?;
            return Ok(CliOutput::stdout(json));
        }
        return Ok(CliOutput::stdout(format!(
            "Sona is already up to date (v{}, channel: {}). Use --force to reinstall.\n",
            check_result.current_version, check_result.channel
        )));
    }

    // Determine target package download path
    let temp_dir;
    let target_file = if let Some(out_path) = args.output.clone() {
        out_path
    } else {
        temp_dir = tempfile::tempdir().map_err(|e| CliError::Io(e.to_string()))?;
        let filename = filename_from_url(&check_result.asset_url);
        temp_dir.path().join(filename)
    };

    // If full install and not --yes, ask for confirmation
    if !args.download_only && !args.yes {
        let prompt = format!(
            "Download and launch Sona Desktop installer v{}? [y/N]: ",
            check_result.latest_version
        );
        write!(io.stderr(), "{prompt}").map_err(|e| CliError::Io(e.to_string()))?;
        let _ = io.stderr().flush();

        let mut answer = String::new();
        let bytes_read = io
            .read_line_stdin(&mut answer)
            .map_err(|e| CliError::Io(format!("Failed to read confirmation: {e}")))?;

        if bytes_read == 0 {
            return Err(CliError::Validation(
                "Update aborted: confirmation required. Run with -y to confirm non-interactively."
                    .to_string(),
            ));
        }

        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Err(CliError::Validation(
                "Update cancelled by user.".to_string(),
            ));
        }
    }

    // Setup cancellation and progress
    let cancel_notify = Arc::new(Notify::new());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64)>();
    let quiet = args.quiet;
    let stderr_is_terminal = io.stderr_is_terminal();
    let display_version = check_result.latest_version.clone();

    let on_progress: Option<Box<dyn FnMut(u64, u64) + Send>> = if quiet {
        None
    } else {
        Some(Box::new(move |downloaded, total| {
            let _ = tx.send((downloaded, total));
        }))
    };

    let download_fut = download_update_package(
        &client,
        &check_result.asset_url,
        &target_file,
        &check_result.signature,
        Some(args.pubkey.as_deref().unwrap_or(DEFAULT_UPDATER_PUBKEY)),
        mirror,
        cancel_notify.clone(),
        on_progress,
    );
    tokio::pin!(download_fut);

    let mut last_pct: Option<i32> = None;
    let mut has_printed = false;

    loop {
        tokio::select! {
            res = &mut download_fut => {
                res.map_err(map_download_error)?;
                break;
            }
            Some((downloaded, total)) = rx.recv() => {
                if quiet || total == 0 {
                    continue;
                }
                let pct = ((downloaded as f64 / total as f64) * 100.0).round() as i32;
                if stderr_is_terminal {
                    write!(io.stderr(), "\rDownloading Sona v{display_version}: {pct}%")
                        .map_err(|e| CliError::Io(e.to_string()))?;
                    let _ = io.stderr().flush();
                    has_printed = true;
                } else if (pct == 100 || pct % 10 == 0) && last_pct != Some(pct) {
                    writeln!(io.stderr(), "Downloading Sona v{display_version}: {pct}%")
                        .map_err(|e| CliError::Io(e.to_string()))?;
                    last_pct = Some(pct);
                }
            }
        }
    }

    if has_printed {
        writeln!(io.stderr()).map_err(|e| CliError::Io(e.to_string()))?;
    }

    if args.download_only {
        if args.json {
            let info = serde_json::json!({
                "status": "downloaded",
                "target_version": check_result.latest_version,
                "package_path": target_file.display().to_string(),
                "verified": true,
            });
            let json = serde_json::to_string_pretty(&info)
                .map_err(|e| CliError::Serialize(e.to_string()))?;
            return Ok(CliOutput::stdout(json));
        }
        return Ok(CliOutput::stdout(format!(
            "Successfully downloaded and verified update package at {}\n",
            target_file.display()
        )));
    }

    // Full install
    let running_exe = std::env::current_exe().map_err(|e| CliError::Io(e.to_string()))?;
    let installed_path =
        install_update_package(&target_file, &running_exe, true).map_err(map_download_error)?;

    if args.json {
        let info = serde_json::json!({
            "status": "installed",
            "previous_version": check_result.current_version,
            "installed_version": check_result.latest_version,
            "channel": check_result.channel,
            "installed_path": installed_path.display().to_string(),
        });
        let json =
            serde_json::to_string_pretty(&info).map_err(|e| CliError::Serialize(e.to_string()))?;
        return Ok(CliOutput::stdout(json));
    }

    Ok(CliOutput::stdout(format!(
        "Successfully updated Sona Desktop to v{}.\n",
        check_result.latest_version
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_arg_converts_to_domain_channel() {
        assert_eq!(
            UpdateChannel::from(UpdateChannelArg::Stable),
            UpdateChannel::Stable
        );
        assert_eq!(
            UpdateChannel::from(UpdateChannelArg::Nightly),
            UpdateChannel::Nightly
        );
    }

    #[test]
    fn filename_from_url_extracts_basename() {
        assert_eq!(
            filename_from_url("https://example.com/downloads/Sona-v1.0.0.exe"),
            "Sona-v1.0.0.exe"
        );
        assert_eq!(
            filename_from_url("https://example.com/downloads/Sona.app.tar.gz?token=abc"),
            "Sona.app.tar.gz"
        );
    }
}
