use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sona_sherpa_directml)");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={lib_dir}");

        if target_os == "windows" && sherpa_lib_dir_has_directml(Path::new(&lib_dir)) {
            println!("cargo:rustc-cfg=sona_sherpa_directml");
        }
    }

    if target_os == "linux" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib/sona");
    } else if target_os == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../Frameworks");
    } else if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=delayimp.lib");
        println!("cargo:rustc-link-arg=/DELAYLOAD:sherpa-onnx-c-api.dll");
    }

    tauri_build::build()
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
