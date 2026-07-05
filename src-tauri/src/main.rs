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

        // Try to attach to parent console first when the desktop binary is
        // launched from a terminal and we want logs to remain visible there.
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            has_console = true;
        } else if show_new_console {
            // Allocate a console to initialize stdout/stderr handles in the C runtime
            // only if we explicitly want to show one without a parent console.
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

/// Escapes `message` so it can be safely embedded inside an AppleScript
/// double-quoted string literal.
///
/// AppleScript string literals cannot contain raw newlines (osascript would
/// raise a syntax error and silently drop the dialog), so newlines are
/// replaced with spaces; backslashes and double quotes are backslash-escaped.
/// Compiled only on macOS and under `cfg(test)`.
#[cfg(any(test, target_os = "macos"))]
fn escape_applescript_text(message: &str) -> String {
    message
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

fn show_error_dialog(message: &str) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        use windows::core::HSTRING;

        let title = HSTRING::from("Sona Startup Error");
        let msg = HSTRING::from(message);
        unsafe {
            MessageBoxW(None, &msg, &title, MB_OK | MB_ICONERROR);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // AppleScript string literals cannot contain raw newlines or unescaped
        // quotes/backslashes; `escape_applescript_text` normalizes all of them.
        let script = format!(
            "display alert \"Sona Startup Error\" message \"{}\" as critical buttons {{\"OK\"}} default button \"OK\"",
            escape_applescript_text(message)
        );
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        // Zenity interprets text as Pango XML markup; escape basic entities to prevent parsing errors
        let escaped_msg = message
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        let res = Command::new("zenity")
            .args([
                "--error",
                "--title=Sona Startup Error",
                &format!("--text={}", escaped_msg),
                "--width=400",
            ])
            .status();

        if let Err(ref e) = res {
            if e.kind() == std::io::ErrorKind::NotFound {
                // Unlike zenity, kdialog renders `--error` text as plain text
                // (no Pango markup), so pass the raw message without escaping.
                let _ = Command::new("kdialog")
                    .args(["--error", message, "--title", "Sona Startup Error"])
                    .status();
            }
        }
    }
}

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
    if std::env::var_os("SONA_TEST_EXIT_BEFORE_APP").is_some() {
        println!("desktop-entry");
        return ExitCode::SUCCESS;
    }

    setup_panic_hook();

    #[cfg(target_os = "windows")]
    fix_console(false);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_applescript_text_replaces_newlines_with_spaces() {
        // Regression: raw newlines break AppleScript string literals and would
        // silently suppress the error dialog, so they must be removed before
        // embedding. Each newline char maps to a single space (CRLF yields two).
        let escaped = escape_applescript_text("a\nb\rc\r\nd");
        assert!(!escaped.contains('\n'));
        assert!(!escaped.contains('\r'));
        assert_eq!(escaped, "a b c  d");

        // A single newline character maps one-to-one to a space.
        assert_eq!(escape_applescript_text("line1\nline2"), "line1 line2");
        assert_eq!(escape_applescript_text("line1\rline2"), "line1 line2");
    }

    #[test]
    fn escape_applescript_text_escapes_quotes_and_backslashes() {
        let escaped = escape_applescript_text(r#"she said "hi" C:\path\end"#);
        assert_eq!(escaped, r#"she said \"hi\" C:\\path\\end"#);
    }
}
