use std::env;
use std::path::Path;

const SHERPA_ONNX_STATIC_LIBS: &[&str] = &[
    "sherpa-onnx-c-api",
    "sherpa-onnx-core",
    "kaldi-decoder-core",
    "sherpa-onnx-kaldifst-core",
    "sherpa-onnx-fstfar",
    "sherpa-onnx-fst",
    "kaldi-native-fbank-core",
    "kissfft-float",
    "piper_phonemize",
    "espeak-ng",
    "ucd",
    "onnxruntime",
    "ssentencepiece_core",
];

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sona_sherpa_directml)");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    embed_windows_test_manifest();

    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os == "windows" && sherpa_lib_dir_has_directml(Path::new(&lib_dir)) {
            println!("cargo:rustc-cfg=sona_sherpa_directml");
        }

        if target_os == "linux" || target_os == "macos" || target_os == "windows" {
            configure_static_sherpa_linking(&lib_dir, &target_os);
        }
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

fn configure_static_sherpa_linking(lib_dir: &str, target_os: &str) {
    let lib_path = Path::new(lib_dir);
    if !lib_path.exists() {
        panic!("SHERPA_ONNX_LIB_DIR does not exist: {}", lib_dir);
    }

    let extension = if target_os == "windows" { "lib" } else { "a" };
    for lib in SHERPA_ONNX_STATIC_LIBS {
        let lib_file = lib_path.join(format!("lib{lib}.{extension}"));
        let lib_file = if target_os == "windows" {
            lib_path.join(format!("{lib}.{extension}"))
        } else {
            lib_file
        };
        if !lib_file.exists() {
            panic!(
                "Missing required sherpa-onnx static library: {}",
                lib_file.display()
            );
        }
        println!("cargo:rustc-link-lib=static={lib}");
    }

    match target_os {
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=dl");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=Foundation");
        }
        _ => {}
    }
}
