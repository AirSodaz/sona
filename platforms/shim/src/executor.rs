use crate::registry::Channel;
use std::path::Path;

pub fn execute(target_path: &Path, args: &[String], channel: Channel) -> ! {
    unsafe {
        std::env::set_var("SONA_CHANNEL", channel.as_str());
        if std::env::var("SONA_IPC_ENDPOINT").is_err() {
            std::env::set_var("SONA_IPC_ENDPOINT", channel.default_ipc_endpoint());
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(target_path);
        cmd.args(args);
        let err = cmd.exec();
        eprintln!(
            "sona-shim: Failed to execute {}: {}",
            target_path.display(),
            err
        );
        std::process::exit(1);
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::TRUE;
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
        let mut child = match std::process::Command::new(target_path).args(args).spawn() {
            Ok(child) => child,
            Err(err) => {
                eprintln!(
                    "sona-shim: Failed to launch {} ({})",
                    target_path.display(),
                    err
                );
                std::process::exit(1);
            }
        };

        // Disable Ctrl+C handling in parent while waiting so child handles signals directly
        unsafe {
            SetConsoleCtrlHandler(None, TRUE);
        }

        let status = child.wait().unwrap_or_else(|_| std::process::exit(1));
        std::process::exit(status.code().unwrap_or(1));
    }

    #[cfg(not(any(windows, unix)))]
    {
        eprintln!("sona-shim: Unsupported platform");
        std::process::exit(1);
    }
}

pub fn quote_windows_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if !arg.contains([' ', '\t', '\n', '\x0b', '\"']) {
        return arg.to_string();
    }

    let mut result = String::with_capacity(arg.len() + 2);
    result.push('\"');

    let mut backslashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else if c == '\"' {
            result.push_str(&"\\".repeat(2 * backslashes + 1));
            backslashes = 0;
            result.push('\"');
        } else {
            result.push_str(&"\\".repeat(backslashes));
            backslashes = 0;
            result.push(c);
        }
    }
    result.push_str(&"\\".repeat(2 * backslashes));
    result.push('\"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_windows_arg() {
        assert_eq!(quote_windows_arg("simple"), "simple");
        assert_eq!(quote_windows_arg("with space"), "\"with space\"");
        assert_eq!(quote_windows_arg(""), "\"\"");
        assert_eq!(quote_windows_arg("quote\"test"), "\"quote\\\"test\"");
    }
}
