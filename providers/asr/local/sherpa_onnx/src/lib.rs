pub mod adapter;
pub mod aligner;
pub mod audio;
pub mod audio_stream;
pub mod batch;
pub mod gpu;
pub mod punctuation;
pub mod recognizer;
pub mod runtime;
pub mod speaker;
pub mod speaker_processing;
pub mod stream_transcriber;
pub mod streaming;

pub use adapter::SherpaOnnxAdapter;

#[cfg(target_os = "windows")]
pub(crate) fn ensure_windows_runtime_dlls() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let Ok(exe_path) = std::env::current_exe() else {
            return;
        };
        if exe_path
            .parent()
            .is_some_and(|dir| dir.join("onnxruntime.dll").is_file())
        {
            return;
        }

        let mut candidate_dirs = Vec::new();
        if let Some(lib_dir) = std::env::var_os("SHERPA_ONNX_LIB_DIR") {
            candidate_dirs.push(std::path::PathBuf::from(lib_dir));
        }
        if let Some(profile_dir) = exe_path
            .ancestors()
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("build"))
            .and_then(|p| p.parent())
        {
            candidate_dirs.push(profile_dir.join("deps"));
            candidate_dirs.push(profile_dir.to_path_buf());
        }

        let Some(dll_path) = candidate_dirs
            .into_iter()
            .map(|dir| dir.join("onnxruntime.dll"))
            .find(|path| path.is_file())
        else {
            return;
        };

        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = dll_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe extern "system" {
            fn LoadLibraryW(lpLibFileName: *const u16) -> *mut std::ffi::c_void;
        }
        // SAFETY: `wide` is a NUL-terminated UTF-16 path pointing to an existing DLL.
        unsafe {
            let _ = LoadLibraryW(wide.as_ptr());
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn ensure_windows_runtime_dlls() {}
