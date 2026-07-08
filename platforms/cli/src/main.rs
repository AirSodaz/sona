use std::process::ExitCode;

fn main() -> ExitCode {
    init_shared_library_directory();

    match sona_cli::run_cli_from_args(std::env::args_os()) {
        Ok(output) => {
            if !output.stdout.is_empty() {
                println!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprintln!("{}", output.stderr);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

#[cfg(target_os = "windows")]
fn init_shared_library_directory() {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::LibraryLoader::SetDllDirectoryW;
    use windows::core::PCWSTR;

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|path| path.to_path_buf()))
    {
        for path in [
            exe_dir.join("../shared_libs"),
            exe_dir.join("shared_libs"),
            exe_dir.join("../resources/shared_libs"),
            exe_dir.join("resources/shared_libs"),
        ] {
            if path.exists() {
                let mut path_u16: Vec<u16> = path.as_os_str().encode_wide().collect();
                path_u16.push(0);
                unsafe {
                    let _ = SetDllDirectoryW(PCWSTR(path_u16.as_ptr()));
                }
                break;
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn init_shared_library_directory() {}
