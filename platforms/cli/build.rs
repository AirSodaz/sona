use std::env;
use std::path::Path;

fn main() {
    inject_build_info();
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");

    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={lib_dir}");
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "linux" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib/sona");
    } else if target_os == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../Frameworks");
    } else if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=delayimp.lib");
        println!("cargo:rustc-link-arg=/DELAYLOAD:sherpa-onnx-c-api.dll");
    }
}

fn inject_build_info() {
    println!("cargo:rerun-if-changed=../../scripts/get-build-info.mjs");
    println!("cargo:rerun-if-env-changed=SONA_CHANNEL");
    println!("cargo:rerun-if-env-changed=CI");
    println!("cargo:rerun-if-env-changed=GITHUB_ACTIONS");
    println!("cargo:rerun-if-env-changed=GITHUB_REF");
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_NUMBER");
    println!("cargo:rerun-if-env-changed=SONA_NIGHTLY");
    println!("cargo:rerun-if-env-changed=SONA_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=SONA_IS_DIRTY");
    println!("cargo:rerun-if-env-changed=SONA_BUILD_NUMBER");
    println!("cargo:rerun-if-env-changed=SONA_BUILD_TIMESTAMP");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let script_path = Path::new(&manifest_dir).join("../../scripts/get-build-info.mjs");

    let output = std::process::Command::new("node")
        .arg(&script_path)
        .arg("--cargo")
        .output();

    if output.as_ref().is_ok_and(|out| out.status.success()) {
        let out = output.unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("cargo:") {
                println!("{trimmed}");
            }
        }
        return;
    }

    // Fallback if node or script is unavailable
    let pkg_version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    println!("cargo:rustc-env=SONA_BUILD_SEMVER={pkg_version}");
    println!("cargo:rustc-env=SONA_BUILD_CHANNEL=dev");
    println!("cargo:rustc-env=SONA_COMMIT_SHA=unknown");
    println!("cargo:rustc-env=SONA_IS_DIRTY=false");
    println!("cargo:rustc-env=SONA_BUILD_NUMBER=");
    println!("cargo:rustc-env=SONA_BUILD_TIMESTAMP=");
    println!("cargo:rustc-env=SONA_DISPLAY_VERSION={pkg_version}+dev.unknown");
    println!("cargo:rustc-env=SONA_IS_UPDATE_SUPPORTED=false");
}
