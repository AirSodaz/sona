use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub fn resolve_target_exe(shim_exe: &Path) -> Result<PathBuf, String> {
    let bin_dir = shim_exe
        .parent()
        .ok_or_else(|| "cannot determine parent directory of executable".to_string())?;
    let root_dir = bin_dir
        .parent()
        .ok_or_else(|| "cannot determine root installation directory".to_string())?;
    let file_name = shim_exe
        .file_name()
        .ok_or_else(|| "cannot determine executable file name".to_string())?;
    let target = root_dir.join(file_name);
    if target == shim_exe {
        return Err("target executable resolves to the shim binary itself".to_string());
    }
    Ok(target)
}

fn main() -> ExitCode {
    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("sona-bin-shim: failed to determine current executable path: {err}");
            return ExitCode::FAILURE;
        }
    };

    let target_exe = match resolve_target_exe(&current_exe) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("sona-bin-shim: {err}");
            return ExitCode::FAILURE;
        }
    };

    if target_exe == current_exe
        || target_exe.canonicalize().ok() == current_exe.canonicalize().ok()
    {
        eprintln!(
            "sona-bin-shim: target executable resolves to the shim binary itself: {}",
            target_exe.display()
        );
        return ExitCode::FAILURE;
    }

    if !target_exe.is_file() {
        eprintln!(
            "sona-bin-shim: target executable not found: {}",
            target_exe.display()
        );
        return ExitCode::FAILURE;
    }

    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    let mut command = std::process::Command::new(&target_exe);
    command.args(&args);
    command.stdin(std::process::Stdio::inherit());
    command.stdout(std::process::Stdio::inherit());
    command.stderr(std::process::Stdio::inherit());

    match command.status() {
        Ok(status) => {
            if let Some(code) = status.code() {
                ExitCode::from(code as u8)
            } else {
                ExitCode::FAILURE
            }
        }
        Err(err) => {
            eprintln!(
                "sona-bin-shim: failed to launch target executable {}: {}",
                target_exe.display(),
                err
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_target_exe() {
        let fake_shim = Path::new("C:\\Program Files\\Sona\\bin\\sona-cli.exe");
        let target = resolve_target_exe(fake_shim).unwrap();
        assert_eq!(
            target,
            PathBuf::from("C:\\Program Files\\Sona\\sona-cli.exe")
        );

        let fake_mcp = Path::new("/opt/sona/bin/sona-mcp");
        let target_mcp = resolve_target_exe(fake_mcp).unwrap();
        assert_eq!(target_mcp, PathBuf::from("/opt/sona/sona-mcp"));
    }

    #[test]
    fn test_resolve_target_exe_rejects_self() {
        let root_shim = Path::new("C:\\sona-cli.exe");
        assert!(resolve_target_exe(root_shim).is_err());
    }
}
