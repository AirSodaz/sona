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
            if let Ok(conout) = OpenOptions::new().write(true).open("CONOUT$") {
                let handle = conout.as_raw_handle() as *mut std::ffi::c_void;
                SetStdHandle(STD_OUTPUT_HANDLE, handle);
                SetStdHandle(STD_ERROR_HANDLE, handle);
                std::mem::forget(conout); // Leak handle so it stays open for the lifetime of the process
            }

            // Redirect stdin
            if let Ok(conin) = OpenOptions::new().read(true).open("CONIN$") {
                let handle = conin.as_raw_handle() as *mut std::ffi::c_void;
                SetStdHandle(STD_INPUT_HANDLE, handle);
                std::mem::forget(conin); // Leak handle so it stays open for the lifetime of the process
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
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

    tauri_appsona_lib::run();
    ExitCode::SUCCESS
}
