// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn redirect_c_stdout_stderr() {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let mut log_dir = std::path::PathBuf::from(local_app_data);
        log_dir.push("com.asoda.sona");
        log_dir.push("logs");
        if std::fs::create_dir_all(&log_dir).is_ok() {
            let mut log_file = log_dir;
            log_file.push("sherpa_logs.txt");

            use std::os::windows::ffi::OsStrExt;
            let path_wide: Vec<u16> = log_file
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mode_wide: Vec<u16> = std::ffi::OsStr::new("a")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                extern "C" {
                    fn __acrt_iob_func(index: std::ffi::c_uint) -> *mut std::ffi::c_void;
                    fn _wfreopen_s(
                        pFile: *mut *mut std::ffi::c_void,
                        path: *const u16,
                        mode: *const u16,
                        stream: *mut std::ffi::c_void,
                    ) -> std::ffi::c_int;
                }

                let mut dummy = std::ptr::null_mut();
                // 1 is stdout, 2 is stderr
                _wfreopen_s(
                    &mut dummy,
                    path_wide.as_ptr(),
                    mode_wide.as_ptr(),
                    __acrt_iob_func(1),
                );
                _wfreopen_s(
                    &mut dummy,
                    path_wide.as_ptr(),
                    mode_wide.as_ptr(),
                    __acrt_iob_func(2),
                );
            }
        }
    }
}

/// Entry point for the Tauri application.
///
/// Delegates execution to the `run` function in the library crate.
fn main() {
    #[cfg(target_os = "windows")]
    redirect_c_stdout_stderr();

    tauri_appsona_lib::run()
}
