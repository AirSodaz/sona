// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;
use tauri_appsona_lib::platform::startup_dialog::show_error_dialog;

fn try_write_emergency_crash_log(message: &str, backtrace: &std::backtrace::Backtrace) {
    use std::io::Write;
    let roots = sona_runtime_fs::default_desktop_app_data_roots();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let log_entry = format!("[{now}] [PANIC_EARLY_ABORT] {message}\n{backtrace}\n");

    let mut written = false;
    for root in roots {
        let logs_dir = root.join("logs");
        if std::fs::create_dir_all(&logs_dir).is_ok() {
            let log_file = logs_dir.join("appsona.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file)
            {
                let _ = writeln!(file, "{log_entry}");
                written = true;
                break;
            }
        }
    }
    if !written
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crash.log")
    {
        let _ = writeln!(file, "{log_entry}");
    }
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(move |info| {
        let (message, location) = sona_core::runtime::logging::format_panic_message(info);
        let backtrace = std::backtrace::Backtrace::capture();
        let error_msg = format!("{}{}\n\nPlease report this issue.", message, location);

        eprintln!("Application panicked: {error_msg}\n{backtrace}");
        log::error!("Application panicked: {message}{location}\n{backtrace}");

        try_write_emergency_crash_log(&error_msg, &backtrace);

        show_error_dialog(&error_msg);
    }));
}

#[tokio::main]
async fn main() -> ExitCode {
    if tauri_appsona_lib::platform::startup_env::should_exit_before_app() {
        println!("desktop-entry");
        return ExitCode::SUCCESS;
    }

    setup_panic_hook();

    #[cfg(target_os = "windows")]
    tauri_appsona_lib::platform::startup_console::fix_console(false);

    match tauri_appsona_lib::run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Tauri startup failure: {err}");
            // The early test exit above is the only non-GUI path here, so show
            // the startup error dialog directly for real desktop launches.
            show_error_dialog(&err.to_string());
            ExitCode::FAILURE
        }
    }
}
