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
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);

        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os == "linux" || target_os == "macos" || target_os == "windows" {
            configure_static_sherpa_linking(&lib_dir, &target_os);
        }
    }

    tauri_build::build()
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
