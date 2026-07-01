use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sona_sherpa_directml)");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    embed_windows_test_manifest();

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    let copied_libs = if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);

        if target_os == "windows" && sherpa_lib_dir_has_directml(Path::new(&lib_dir)) {
            println!("cargo:rustc-cfg=sona_sherpa_directml");
        }

        // Copy shared libraries to resources/shared_libs
        copy_shared_libs(&lib_dir, &target_os);
        true
    } else {
        // Attempt to copy from cargo's target output directory if SHERPA_ONNX_LIB_DIR is not set
        copy_from_target_dir(&target_os)
    };

    if !copied_libs {
        // Ensure the resources/shared_libs directory exists and is not empty so Tauri doesn't fail
        let dest_dir = Path::new("resources/shared_libs");
        std::fs::create_dir_all(dest_dir).ok();
        std::fs::write(dest_dir.join(".placeholder"), "").ok();
    }

    // Configure rpaths for dynamic linking
    if target_os == "linux" {
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN:$ORIGIN/../lib/sona/resources/shared_libs:$ORIGIN/../lib64/sona/resources/shared_libs"
        );
    } else if target_os == "macos" {
        // Configure loader_path for local cargo running, and resources folder path for Sona.app bundle
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../Resources/resources/shared_libs");
    } else if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=delayimp.lib");
        println!("cargo:rustc-link-arg=/DELAYLOAD:sherpa-onnx-c-api.dll");
    }

    tauri_build::build()
}

#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn embed_windows_test_manifest() {
    let out_dir =
        env::var("OUT_DIR").expect("OUT_DIR is required to compile Windows test resources");
    println!("cargo:rerun-if-changed=windows-test-manifest.rc");
    println!("cargo:rerun-if-changed=windows-test.exe.manifest");
    println!("cargo:rustc-link-search=native={out_dir}");
    embed_resource::compile_for_tests("windows-test-manifest.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed Common Controls v6 manifest for Rust test binaries");
}

fn sherpa_lib_dir_has_directml(lib_dir: &Path) -> bool {
    [
        "onnxruntime_providers_dml.lib",
        "DirectML.lib",
        "DirectML.dll",
        "directml.lib",
        "directml.dll",
    ]
    .iter()
    .any(|file| lib_dir.join(file).exists())
}

fn find_target_dir() -> Option<PathBuf> {
    let out_dir = env::var("OUT_DIR").ok()?;
    let mut path = PathBuf::from(out_dir);
    // OUT_DIR is target/<profile>/build/sona-<hash>/out
    // We go up 4 times to reach target/<profile>/
    for _ in 0..4 {
        path.pop();
    }
    if path.exists() { Some(path) } else { None }
}

fn copy_from_target_dir(target_os: &str) -> bool {
    if let Some(target_dir) = find_target_dir() {
        let dest_dir = Path::new("resources/shared_libs");
        std::fs::create_dir_all(dest_dir).ok();

        let mut copied = false;
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                        let should_copy = if target_os == "windows" {
                            filename == "sherpa-onnx-c-api.dll" || filename == "onnxruntime.dll"
                        } else if target_os == "macos" {
                            filename == "libsherpa-onnx-c-api.dylib"
                                || filename == "libonnxruntime.dylib"
                        } else if target_os == "linux" {
                            filename.starts_with("libsherpa-onnx-c-api.so")
                                || filename.starts_with("libonnxruntime.so")
                        } else {
                            false
                        };
                        if should_copy {
                            let dest_path = dest_dir.join(filename);
                            if std::fs::copy(&path, &dest_path).is_ok() {
                                copied = true;
                                println!("cargo:rerun-if-changed={}", path.display());
                            }
                        }
                    }
                }
            }
        }
        copied
    } else {
        false
    }
}

fn copy_shared_libs(lib_dir: &str, target_os: &str) {
    let lib_path = Path::new(lib_dir);
    let dest_dir = Path::new("resources/shared_libs");

    // Create destination directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(dest_dir) {
        panic!("Failed to create shared_libs directory: {}", e);
    }

    // Clean existing files in resources/shared_libs to avoid mixing versions
    // Files locked by a running process are silently skipped
    if let Ok(entries) = std::fs::read_dir(dest_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                if let Err(e) = std::fs::remove_file(entry.path()) {
                    if e.kind() != std::io::ErrorKind::PermissionDenied {
                        eprintln!(
                            "Warning: could not remove {}: {}",
                            entry.path().display(),
                            e
                        );
                    }
                }
            }
        }
    }

    // Write placeholder to ensure Tauri glob matches
    std::fs::write(dest_dir.join(".placeholder"), "").ok();

    // Copy DLLs/dylib/so files
    if let Ok(entries) = std::fs::read_dir(lib_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    let should_copy = if target_os == "windows" {
                        filename.ends_with(".dll")
                    } else if target_os == "macos" {
                        filename.ends_with(".dylib") || filename.contains(".dylib.")
                    } else if target_os == "linux" {
                        filename.ends_with(".so") || filename.contains(".so.")
                    } else {
                        false
                    };
                    if should_copy {
                        let dest_path = dest_dir.join(filename);
                        if let Err(e) = std::fs::copy(&path, &dest_path) {
                            if e.kind() == std::io::ErrorKind::PermissionDenied {
                                eprintln!(
                                    "Warning: could not replace {} (in use by another process). Old version retained.",
                                    filename
                                );
                            } else {
                                panic!(
                                    "Failed to copy shared library {} to {}: {}",
                                    path.display(),
                                    dest_path.display(),
                                    e
                                );
                            }
                        } else {
                            println!("cargo:rerun-if-changed={}", path.display());
                        }
                    }
                }
            }
        }
    }
}
