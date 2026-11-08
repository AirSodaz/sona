//! Sona Desktop - Pure Rust Native Application Entry Point
//!
//! Milestone 1: Application Entry & Tokio Bridge (Feature 3)

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sona_desktop_lib::platform::startup_dialog::show_error_dialog;
use std::process::ExitCode;

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
        let backtrace = std::backtrace::Backtrace::capture();
        let error_msg = format!(
            "{}{}\n\n{}\n\nPlease report this issue.",
            message, location, backtrace
        );

        eprintln!("Application panicked: {error_msg}");
        let _ = std::fs::write("panic.log", &error_msg);

        show_error_dialog(&format!(
            "{}{}\n\nPlease report this issue.",
            message, location
        ));
    }));
}

fn main() -> ExitCode {
    if sona_desktop_lib::platform::startup_env::should_exit_before_app() {
        println!("desktop-entry");
        return ExitCode::SUCCESS;
    }

    setup_panic_hook();

    #[cfg(target_os = "windows")]
    sona_desktop_lib::platform::startup_console::fix_console(false);

    match sona_desktop_lib::run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Desktop startup failure: {err}");
            show_error_dialog(&err.to_string());
            ExitCode::FAILURE
        }
    }
}
