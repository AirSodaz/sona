// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

#[cfg(target_os = "windows")]
fn fix_console() {
    unsafe {
        #[link(name = "kernel32")]
        extern "system" {
            fn AllocConsole() -> i32;
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }
        #[link(name = "user32")]
        extern "system" {
            fn ShowWindow(hwnd: *mut std::ffi::c_void, cmd: i32) -> i32;
        }

        // Allocate a console to natively initialize stdout/stderr handles in the C runtime
        if AllocConsole() != 0 {
            let hwnd = GetConsoleWindow();
            if !hwnd.is_null() {
                // Instantly hide the console window
                ShowWindow(hwnd, 0);
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();

    if tauri_appsona_lib::cli::should_run_cli(args.get(1..).unwrap_or(&[])) {
        #[cfg(target_os = "windows")]
        fix_console();

        return match tauri_appsona_lib::cli::run_cli_from_args(args).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }

    #[cfg(target_os = "windows")]
    fix_console();

    tauri_appsona_lib::run();
    ExitCode::SUCCESS
}
