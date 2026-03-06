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
            // Copy dlls to the output directory
            let out_dir = env::var("OUT_DIR").unwrap();
            let out_path = Path::new(&out_dir)
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap();

            let lib_path = Path::new(&lib_dir);
            if lib_path.exists() {
                for entry in fs::read_dir(lib_path).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("dll") {
                        let file_name = path.file_name().unwrap();
                        let dest_path = out_path.join(file_name);
                        fs::copy(&path, &dest_path).unwrap();
                    }
                }
            }
        }
    }

    tauri_build::build()
}
