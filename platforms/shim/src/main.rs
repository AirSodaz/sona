use sona_shim::dispatcher::{detect_role, resolve_target_channel, resolve_target_executable};
use sona_shim::executor::execute;
use sona_shim::installer::{check_path_status, install_shims, normalize_path, uninstall_shims};
use sona_shim::registry::{
    Channel, TargetRole, find_executable, get_default_channel, get_registered_channel,
    set_default_channel,
};
use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let program_name = args
        .first()
        .cloned()
        .unwrap_or_else(|| "sona-shim".to_string());
    if !args.is_empty() {
        args.remove(0); // strip program name
    }

    let role = detect_role(&program_name);

    match role {
        TargetRole::Shim => {
            handle_shim_subcommand(&args);
        }
        TargetRole::Cli | TargetRole::Mcp => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let channel = resolve_target_channel(&program_name, &mut args, &cwd, role);

            let target_exe = resolve_target_executable(role, channel);
            match target_exe {
                Some(path) => {
                    execute(&path, &args, channel);
                }
                None => {
                    eprintln!(
                        "Error: Sona executable for {} ({}) was not found.",
                        channel.display_name(),
                        role.binary_name()
                    );
                    eprintln!(
                        "Please ensure Sona is installed or configure the installation directory via:"
                    );
                    eprintln!(
                        "  sona-shim install --channel {} --path <INSTALL_DIR>",
                        channel.as_str()
                    );
                    eprintln!("Download releases at: https://github.com/AirSodaz/sona");
                    std::process::exit(1);
                }
            }
        }
    }
}

fn handle_shim_subcommand(args: &[String]) {
    let subcmd = args.first().map(|s| s.as_str()).unwrap_or("help");

    match subcmd {
        "help" | "--help" | "-h" => {
            print_help();
        }
        "version" | "--version" | "-V" => {
            println!("sona-shim {}", env!("CARGO_PKG_VERSION"));
        }
        "list" => {
            handle_list(args);
        }
        "which" => {
            handle_which(args);
        }
        "default" => {
            handle_default(args);
        }
        "install" => {
            handle_install(args);
        }
        "uninstall" => {
            handle_uninstall(args);
        }
        "status" => {
            handle_status(args);
        }
        other => {
            eprintln!(
                "Unknown command: '{}'. Run 'sona-shim --help' for available commands.",
                other
            );
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        r#"Sona Universal Dispatcher & Shim (sona-shim) {}

USAGE:
    sona-shim <COMMAND> [OPTIONS]
    sona-cli [+stable|+nightly] [ARGS]...
    sona-mcp [+stable|+nightly] [ARGS]...

COMMANDS:
    list             List detected Sona installations, channels, and versions
    which <TARGET>   Display the resolved path to a target binary (cli or mcp)
    default <CHAN>   Set the global default channel ('stable' or 'nightly')
    install          Install universal shims into user PATH (~/.sona/bin)
    uninstall        Uninstall shims or unregister a specific channel
    status           Show current PATH configuration and shim status
    help             Show this help message
    version          Show version information

OPTIONS:
    --json           Format output as JSON (for list, status)
"#,
        env!("CARGO_PKG_VERSION")
    );
}

fn handle_list(args: &[String]) {
    let json_output = args.iter().any(|a| a == "--json");
    let default_ch = get_default_channel();

    let channels = [Channel::Stable, Channel::Nightly];
    let mut channel_reports = Vec::new();

    for ch in &channels {
        let reg = get_registered_channel(*ch);
        let cli = find_executable(*ch, TargetRole::Cli);
        let mcp = find_executable(*ch, TargetRole::Mcp);
        let is_installed = cli.is_some() || mcp.is_some();

        channel_reports.push(serde_json::json!({
            "channel": ch.as_str(),
            "name": ch.display_name(),
            "is_default": *ch == default_ch,
            "installed": is_installed,
            "registered_path": reg.as_ref().map(|r| r.install_path.to_string_lossy()),
            "cli_executable": cli.map(|p| p.to_string_lossy().to_string()),
            "mcp_executable": mcp.map(|p| p.to_string_lossy().to_string()),
            "version": reg.and_then(|r| r.version),
        }));
    }

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&channel_reports).unwrap_or_default()
        );
    } else {
        println!("Sona Installed Channels & Dispatching Status:");
        println!("Default Channel: {}\n", default_ch.display_name());
        for entry in &channel_reports {
            let name = entry["name"].as_str().unwrap_or_default();
            let is_default = entry["is_default"].as_bool().unwrap_or(false);
            let installed = entry["installed"].as_bool().unwrap_or(false);
            let marker = if is_default { " (default)" } else { "" };
            let status = if installed { "Available" } else { "Not found" };

            println!("  * {}{}: [{}]", name, marker, status);
            if let Some(cli) = entry["cli_executable"].as_str() {
                println!("      cli: {}", cli);
            }
            if let Some(mcp) = entry["mcp_executable"].as_str() {
                println!("      mcp: {}", mcp);
            }
            if let Some(ver) = entry["version"].as_str() {
                println!("      version: {}", ver);
            }
            println!();
        }
    }
}

fn handle_which(args: &[String]) {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: sona-shim which <sona-cli|sona-mcp|cli|mcp> [+stable|+nightly]");
        return;
    }
    if args.len() < 2 {
        eprintln!("Usage: sona-shim which <sona-cli|sona-mcp|cli|mcp> [+stable|+nightly]");
        std::process::exit(1);
    }
    let target_arg = &args[1];
    let role = match target_arg.to_lowercase().as_str() {
        "sona-cli" | "cli" => TargetRole::Cli,
        "sona-mcp" | "mcp" => TargetRole::Mcp,
        _ => {
            eprintln!(
                "Invalid target '{}'. Expected <sona-cli|sona-mcp|cli|mcp>.",
                target_arg
            );
            std::process::exit(1);
        }
    };

    let mut remaining: Vec<String> = args[2..].to_vec();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let channel = resolve_target_channel(target_arg, &mut remaining, &cwd, role);

    match resolve_target_executable(role, channel) {
        Some(path) => {
            println!("{}", path.to_string_lossy());
        }
        None => {
            eprintln!(
                "Error: No executable resolved for {} on channel '{}'.",
                role.binary_name(),
                channel.as_str()
            );
            std::process::exit(1);
        }
    }
}

fn handle_default(args: &[String]) {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: sona-shim default <stable|nightly>");
        return;
    }
    if args.len() < 2 {
        eprintln!("Usage: sona-shim default <stable|nightly>");
        std::process::exit(1);
    }
    let Some(channel) = Channel::parse(&args[1]) else {
        eprintln!("Invalid channel '{}'. Use 'stable' or 'nightly'.", args[1]);
        std::process::exit(1);
    };

    if let Err(e) = set_default_channel(channel) {
        eprintln!("Error setting default channel: {}", e);
        std::process::exit(1);
    }

    println!("Default channel set to: {}", channel.display_name());
}

fn handle_install(args: &[String]) {
    let mut channel_opt = None;
    let mut path_opt = None;
    let mut ver_opt = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!(
                    "Usage: sona-shim install [--channel <stable|nightly>] [--path <PATH>] [--version <VERSION>]"
                );
                return;
            }
            "--channel" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: Option '--channel' requires an argument.");
                    std::process::exit(1);
                }
                let parsed = Channel::parse(&args[i + 1]);
                if parsed.is_none() {
                    eprintln!(
                        "Invalid channel '{}'. Use 'stable' or 'nightly'.",
                        args[i + 1]
                    );
                    std::process::exit(1);
                }
                channel_opt = parsed;
                i += 2;
            }
            "--path" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: Option '--path' requires an argument.");
                    std::process::exit(1);
                }
                path_opt = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--version" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: Option '--version' requires an argument.");
                    std::process::exit(1);
                }
                ver_opt = Some(args[i + 1].clone());
                i += 2;
            }
            unknown => {
                eprintln!(
                    "Unknown option '{}'. Run 'sona-shim install --help' for usage.",
                    unknown
                );
                std::process::exit(1);
            }
        }
    }
    // If channel is not explicitly specified, infer from path or current_exe location
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("sona-shim"));
    if channel_opt.is_none() {
        if let Some(p) = &path_opt {
            if p.join("Sona-Nightly.exe").is_file()
                || p.join("sona-nightly.exe").is_file()
                || p.join("Sona-Nightly.app").exists()
            {
                channel_opt = Some(Channel::Nightly);
            } else if p.join("Sona.exe").is_file()
                || p.join("sona.exe").is_file()
                || p.join("Sona.app").exists()
            {
                channel_opt = Some(Channel::Stable);
            } else if p.to_string_lossy().to_lowercase().contains("nightly") {
                channel_opt = Some(Channel::Nightly);
            } else {
                channel_opt = Some(Channel::Stable);
            }
        } else if current_exe
            .to_string_lossy()
            .to_lowercase()
            .contains("nightly")
        {
            channel_opt = Some(Channel::Nightly);
        }
    }

    // If channel is provided with path, prepare registration tuple
    let reg_tuple = if let (Some(ch), Some(p)) = (channel_opt, path_opt.as_ref()) {
        Some((ch, p.as_path(), ver_opt.as_deref(), None))
    } else {
        None
    };

    match install_shims(&current_exe, reg_tuple) {
        Ok(report) => {
            println!("Universal Sona shims installed successfully!");
            println!("  Shim Directory: {}", report.shim_dir.display());
            println!("  Created Shims: {}", report.created_shims.join(", "));
            if report.updated_path {
                println!(
                    "  User PATH: Added {} to user environment.",
                    report.shim_dir.display()
                );
            } else {
                println!("  User PATH: Already configured.");
            }
            if let Some(ch) = report.registered_channel {
                println!("  Registered Channel: {}", ch);
            }
        }
        Err(e) => {
            eprintln!("Error installing shims: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_uninstall(args: &[String]) {
    let mut channel_opt = None;
    let mut path_opt = None;
    let mut force = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!(
                    "Usage: sona-shim uninstall [--channel <stable|nightly>] [--path <PATH>] [--force]"
                );
                return;
            }
            "--channel" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: Option '--channel' requires an argument.");
                    std::process::exit(1);
                }
                let parsed = Channel::parse(&args[i + 1]);
                if parsed.is_none() {
                    eprintln!(
                        "Invalid channel '{}'. Use 'stable' or 'nightly'.",
                        args[i + 1]
                    );
                    std::process::exit(1);
                }
                channel_opt = parsed;
                i += 2;
            }
            "--path" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: Option '--path' requires an argument.");
                    std::process::exit(1);
                }
                path_opt = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--force" => {
                force = true;
                i += 1;
            }
            unknown => {
                eprintln!(
                    "Unknown option '{}'. Run 'sona-shim uninstall --help' for usage.",
                    unknown
                );
                std::process::exit(1);
            }
        }
    }
    if channel_opt.is_none() && !force {
        if let Some(target_path) = &path_opt {
            let target_norm = normalize_path(target_path);
            if let Some(stable) = get_registered_channel(Channel::Stable)
                && normalize_path(&stable.install_path) == target_norm
            {
                channel_opt = Some(Channel::Stable);
            }
            if let Some(nightly) = get_registered_channel(Channel::Nightly)
                && normalize_path(&nightly.install_path) == target_norm
            {
                channel_opt = Some(Channel::Nightly);
            }
            if channel_opt.is_none() {
                if target_path.join("Sona-Nightly.exe").is_file()
                    || target_path.join("sona-nightly.exe").is_file()
                    || target_path.join("Sona-Nightly.app").exists()
                    || target_path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains("nightly")
                {
                    channel_opt = Some(Channel::Nightly);
                } else if target_path.join("Sona.exe").is_file()
                    || target_path.join("sona.exe").is_file()
                    || target_path.join("Sona.app").exists()
                {
                    channel_opt = Some(Channel::Stable);
                }
            }
        }
        if channel_opt.is_none()
            && let Ok(current_exe) = std::env::current_exe()
        {
            let shim_dir = sona_shim::installer::get_shim_dir();
            let norm_exe = normalize_path(&current_exe);
            let norm_shim = normalize_path(&shim_dir);
            let is_in_shim_dir = norm_exe.starts_with(&norm_shim)
                || current_exe
                    .parent()
                    .is_some_and(|p| normalize_path(p) == norm_shim);
            if !is_in_shim_dir {
                let exe_str = current_exe.to_string_lossy().to_lowercase();
                if exe_str.contains("nightly") {
                    channel_opt = Some(Channel::Nightly);
                } else if let Some(parent) = current_exe.parent() {
                    let parent_name = parent
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if parent_name == "sona" || parent_name == "com.asoda.sona" {
                        channel_opt = Some(Channel::Stable);
                    }
                }
            }
        }
    }

    match uninstall_shims(channel_opt, force) {
        Ok(report) => {
            if let Some(removed) = report.removed_channel {
                println!("Unregistered channel: {}", removed);
            }
            if let Some(remaining) = report.active_remaining_channel {
                println!(
                    "Reference count notice: Channel '{}' remains installed. Shims and PATH retained.",
                    remaining
                );
            } else {
                if report.shims_removed {
                    println!("Removed shims from directory.");
                }
                if report.path_removed {
                    println!("Removed shim directory from user PATH.");
                }
                println!("Sona shims uninstalled completely.");
            }
        }
        Err(e) => {
            eprintln!("Error uninstalling shims: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_status(args: &[String]) {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: sona-shim status [--json]");
        return;
    }
    let json_output = args.iter().any(|a| a == "--json");
    let status = check_path_status();
    let default_ch = get_default_channel();

    if json_output {
        let payload = serde_json::json!({
            "shim_dir": status.shim_dir.to_string_lossy(),
            "in_current_path": status.in_current_path,
            "in_user_path": status.in_user_path,
            "installed_shims": status.installed_shims,
            "default_channel": default_ch.as_str(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
    } else {
        println!("Sona Shim Status:");
        println!("  Directory: {}", status.shim_dir.display());
        println!("  In Current Process PATH: {}", status.in_current_path);
        println!("  In User Environment PATH: {}", status.in_user_path);
        println!("  Default Channel: {}", default_ch.display_name());
        println!("  Installed Shims: {}", status.installed_shims.join(", "));
    }
}
