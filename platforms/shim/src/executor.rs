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
        use windows_sys::Win32::Foundation::{CloseHandle, FALSE, TRUE};
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
        use windows_sys::Win32::System::Threading::{
            CreateProcessW, GetExitCodeProcess, INFINITE, PROCESS_INFORMATION, STARTUPINFOW,
            WaitForSingleObject,
        };

        let app_name_w: Vec<u16> = target_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Build command line: program name followed by arguments
        let mut cmdline = format!("\"{}\"", target_path.to_string_lossy());
        for arg in args {
            cmdline.push(' ');
            cmdline.push_str(&quote_windows_arg(arg));
        }

        let mut cmdline_w: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();

        let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

        // Disable Ctrl+C handling in parent so the child console receives it natively
        unsafe {
            SetConsoleCtrlHandler(None, TRUE);
        }

        let created = unsafe {
            CreateProcessW(
                app_name_w.as_ptr(),
                cmdline_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                TRUE,
                0,
                std::ptr::null_mut(),
                std::ptr::null(), // Inherit current directory
                &si,
                &mut pi,
            )
        };

        if created == FALSE {
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            eprintln!(
                "sona-shim: Failed to launch {} (error code {})",
                target_path.display(),
                err
            );
            std::process::exit(1);
        }

        unsafe {
            WaitForSingleObject(pi.hProcess, INFINITE);
            let mut exit_code: u32 = 0;
            GetExitCodeProcess(pi.hProcess, &mut exit_code);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
            std::process::exit(exit_code as i32);
        }
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
