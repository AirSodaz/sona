// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

#[cfg(target_os = "windows")]
fn fix_console(show_new_console: bool) {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;

    unsafe {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AllocConsole() -> i32;
            fn AttachConsole(dwProcessId: u32) -> i32;
            fn SetStdHandle(nStdHandle: u32, hHandle: *mut std::ffi::c_void) -> i32;
            fn GetLastError() -> u32;
        }

        const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
        const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
        const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;

        let mut has_console = false;

        // Try to attach to parent console first (for CLI usage in terminal)
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            has_console = true;
        } else if show_new_console {
            // Allocate a console to natively initialize stdout/stderr handles in the C runtime
            // only if we explicitly want to show a console (CLI mode launched without parent console)
            if AllocConsole() != 0 {
                has_console = true;
            }
        }

        if has_console {
            // Redirect stdout and stderr
            match OpenOptions::new().write(true).open("CONOUT$") {
                Ok(conout) => {
                    let handle = conout.as_raw_handle();
                    if SetStdHandle(STD_OUTPUT_HANDLE, handle) == 0 {
                        eprintln!(
                            "[debug] Failed to set STD_OUTPUT_HANDLE: GetLastError() = {}",
                            GetLastError()
                        );
                    }
                    if SetStdHandle(STD_ERROR_HANDLE, handle) == 0 {
                        eprintln!(
                            "[debug] Failed to set STD_ERROR_HANDLE: GetLastError() = {}",
                            GetLastError()
                        );
                    }
                    std::mem::forget(conout); // Leak handle so it stays open for the lifetime of the process
                }
                Err(e) => {
                    eprintln!("[debug] Failed to open CONOUT$: {}", e);
                }
            }

            // Redirect stdin
            match OpenOptions::new().read(true).open("CONIN$") {
                Ok(conin) => {
                    let handle = conin.as_raw_handle();
                    if SetStdHandle(STD_INPUT_HANDLE, handle) == 0 {
                        eprintln!(
                            "[debug] Failed to set STD_INPUT_HANDLE: GetLastError() = {}",
                            GetLastError()
                        );
                    }
                    std::mem::forget(conin); // Leak handle so it stays open for the lifetime of the process
                }
                Err(e) => {
                    eprintln!("[debug] Failed to open CONIN$: {}", e);
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn show_error_dialog(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::HSTRING;

    let title = HSTRING::from("Sona Startup Error");
    let msg = HSTRING::from(message);
    unsafe {
        MessageBoxW(None, &msg, &title, MB_OK | MB_ICONERROR);
    }
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
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

        #[cfg(target_os = "windows")]
        show_error_dialog(&error_msg);
    }));
}

#[tokio::main]
async fn main() -> ExitCode {
    setup_panic_hook();

    let args: Vec<_> = std::env::args_os().collect();

    if tauri_appsona_lib::cli::should_run_cli(args.get(1..).unwrap_or(&[])) {
        #[cfg(target_os = "windows")]
        fix_console(true);

        return match tauri_appsona_lib::cli::run_cli_from_args(args).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }

    #[cfg(target_os = "windows")]
    fix_console(false);

    match tauri_appsona_lib::run_app() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Tauri startup failure: {err}");
            #[cfg(target_os = "windows")]
            show_error_dialog(&err.to_string());
            ExitCode::FAILURE
        }
    }
}
