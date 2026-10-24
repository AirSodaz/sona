use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sona_sherpa_directml)");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");

    if let Ok(out_dir) = env::var("OUT_DIR") {
        let out_path = PathBuf::from(out_dir);
        if let Some(profile_dir) = out_path.ancestors().nth(3) {
            let deps_dir = profile_dir.join("deps");
            let _ = fs::create_dir_all(&deps_dir);
            copy_runtime_libs(profile_dir, &deps_dir);

            if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
                let lib_path = PathBuf::from(lib_dir);
                if lib_path.exists() {
                    copy_runtime_libs(&lib_path, &deps_dir);
                }
            }
        }
    }
}

fn copy_runtime_libs(src: &Path, dst: &Path) {
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_runtime_lib = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "dll" || ext == "so" || ext == "dylib")
                || path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|name| name.contains(".so.") || name.contains(".dylib."));
            if is_runtime_lib && let Some(file_name) = path.file_name() {
                let target = dst.join(file_name);
                let _ = fs::copy(&path, target);
            }
        }
    }
}
