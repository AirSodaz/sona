use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");

    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={lib_dir}");
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "linux" {
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN:$ORIGIN/../shared_libs:$ORIGIN/../resources/shared_libs"
        );
    } else if target_os == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../shared_libs");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../Resources/resources/shared_libs");
    } else if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=delayimp.lib");
        println!("cargo:rustc-link-arg=/DELAYLOAD:sherpa-onnx-c-api.dll");
    }
}
