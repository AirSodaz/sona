use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);
        println!("cargo:rustc-link-lib=dylib=sherpa-onnx-c-api");

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os == "linux" || target_os == "macos" {
            println!("cargo:rustc-link-lib=dylib=onnxruntime");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
        } else if target_os == "windows" {
            let lib_path = Path::new(&lib_dir);
            if !lib_path.exists() {
                panic!("SHERPA_ONNX_LIB_DIR does not exist: {}", lib_dir);
            }

            let required_files = [
                "sherpa-onnx-c-api.lib",
                "sherpa-onnx-c-api.dll",
                "onnxruntime.dll",
            ];
            for file_name in required_files {
                let file_path = lib_path.join(file_name);
                if !file_path.exists() {
                    panic!(
                        "Missing required sherpa-onnx Windows file: {}",
                        file_path.display()
                    );
                }
            }

            // Copy dlls to the output directory
            let out_dir = env::var("OUT_DIR").unwrap();
            let out_path = Path::new(&out_dir)
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap();

            for dll in ["sherpa-onnx-c-api.dll", "onnxruntime.dll"] {
                let source_path = lib_path.join(dll);
                let dest_path = out_path.join(dll);
                fs::copy(&source_path, &dest_path).unwrap_or_else(|error| {
                    panic!(
                        "Failed to copy {} to {}: {}",
                        source_path.display(),
                        dest_path.display(),
                        error
                    )
                });
            }
        }
    }

    tauri_build::build()
}
