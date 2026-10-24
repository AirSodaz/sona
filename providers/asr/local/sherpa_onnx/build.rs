use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sona_sherpa_directml)");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Ok(out_dir) = env::var("OUT_DIR")
    {
        let out_path = PathBuf::from(out_dir);
        if let Some(profile_dir) = out_path.ancestors().nth(3) {
            let deps_dir = profile_dir.join("deps");
            let _ = fs::create_dir_all(&deps_dir);
            copy_dlls(profile_dir, &deps_dir);
        }
    }
}

fn copy_dlls(src: &Path, dst: &Path) {
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("dll")
                && let Some(file_name) = path.file_name()
            {
                let target = dst.join(file_name);
                let _ = fs::copy(&path, target);
            }
        }
    }
}
