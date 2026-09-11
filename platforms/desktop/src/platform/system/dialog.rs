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

        if matches!(&res, Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
            // Unlike zenity, kdialog renders `--error` text as plain text
            // (no Pango markup), so pass the raw message without escaping.
            let _ = Command::new("kdialog")
                .args(["--error", message, "--title", "Sona Startup Error"])
                .status();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyDatabaseAction {
    BackupAndReset,
    Exit,
}

pub fn prompt_legacy_database_migration(found: i64, minimum: i64) -> LegacyDatabaseAction {
    let title = "数据库版本不兼容";
    let message = format!(
        "当前数据库架构版本（v{found}）低于支持的最低版本（v{minimum}）。\n\n您可以选择备份现有数据库并重置以继续启动，或者退出程序。\n选择“备份并重置数据库”将会备份旧数据库文件，并重新创建新数据库以供正常使用。"
    );

    let res = rfd::MessageDialog::new()
        .set_title(title)
        .set_description(&message)
        .set_level(rfd::MessageLevel::Warning)
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "备份并重置数据库".to_string(),
            "退出".to_string(),
        ))
        .show();
    resolve_legacy_database_action(&res)
}

pub fn resolve_legacy_database_action(res: &rfd::MessageDialogResult) -> LegacyDatabaseAction {
    match res {
        rfd::MessageDialogResult::Custom(s) if s == "备份并重置数据库" => {
            LegacyDatabaseAction::BackupAndReset
        }
        rfd::MessageDialogResult::Ok => LegacyDatabaseAction::BackupAndReset,
        _ => LegacyDatabaseAction::Exit,
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

    #[test]
    fn test_resolve_legacy_database_action() {
        assert_eq!(
            resolve_legacy_database_action(&rfd::MessageDialogResult::Custom(
                "备份并重置数据库".to_string()
            )),
            LegacyDatabaseAction::BackupAndReset
        );
        assert_eq!(
            resolve_legacy_database_action(&rfd::MessageDialogResult::Ok),
            LegacyDatabaseAction::BackupAndReset
        );
        assert_eq!(
            resolve_legacy_database_action(&rfd::MessageDialogResult::Custom("退出".to_string())),
            LegacyDatabaseAction::Exit
        );
        assert_eq!(
            resolve_legacy_database_action(&rfd::MessageDialogResult::Cancel),
            LegacyDatabaseAction::Exit
        );
        assert_eq!(
            resolve_legacy_database_action(&rfd::MessageDialogResult::No),
            LegacyDatabaseAction::Exit
        );
    }
}
