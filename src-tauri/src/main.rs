// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;
use tauri_appsona_lib::platform::startup_dialog::show_error_dialog;

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "An unexpected panic occurred."
        };
        let location = info
            .location()
            .map(|loc| format!(" at {}:{}", loc.file(), loc.line()))
            .unwrap_or_default();
        let error_msg = format!("{}{}\n\nPlease report this issue.", message, location);

        eprintln!("Application panicked: {error_msg}");

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
