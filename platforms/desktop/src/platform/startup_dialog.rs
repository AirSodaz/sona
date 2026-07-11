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

pub fn show_error_dialog(message: &str) {
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
