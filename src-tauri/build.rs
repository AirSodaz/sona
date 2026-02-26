use std::env;
use std::path::Path;

fn main() {
    if let Ok(lib_dir) = env::var("SHERPA_ONNX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);

        let is_static = Path::new(&lib_dir).join("sherpa-onnx-core.lib").exists()
            || Path::new(&lib_dir).join("libsherpa-onnx-core.a").exists();

        if is_static {
            // Static linking required for sherpa-onnx
            println!("cargo:rustc-link-lib=static=sherpa-onnx-core");
            println!("cargo:rustc-link-lib=static=sherpa-onnx-cxx-api");
            println!("cargo:rustc-link-lib=static=kaldi-native-fbank-core");
            println!("cargo:rustc-link-lib=static=kaldi-decoder-core");
            println!("cargo:rustc-link-lib=static=sherpa-onnx-fst");
            println!("cargo:rustc-link-lib=static=sherpa-onnx-fstfar");
            println!("cargo:rustc-link-lib=static=sherpa-onnx-kaldifst-core");
            println!("cargo:rustc-link-lib=static=ssentencepiece_core");
            println!("cargo:rustc-link-lib=static=piper_phonemize");
            println!("cargo:rustc-link-lib=static=espeak-ng");
            println!("cargo:rustc-link-lib=static=kissfft-float");
            println!("cargo:rustc-link-lib=static=ucd");
            println!("cargo:rustc-link-lib=static=cargs");
        } else {
            // Dynamic linking
            println!("cargo:rustc-link-lib=dylib=sherpa-onnx-c-api");
            println!("cargo:rustc-link-lib=dylib=onnxruntime");

            let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
            if target_os == "linux" || target_os == "macos" {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
            }
        }
    }

    tauri_build::build()
}
