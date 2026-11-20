use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_REF");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_VERSION");

    let llama_ref = env::var("LLAMA_CPP_REF").unwrap_or_else(|_| "b11429".to_string());
    let sherpa_version = env::var("SHERPA_ONNX_VERSION").unwrap_or_else(|_| "2.0.2".to_string());
    println!("cargo:rustc-env=SONA_CUDA_ADDON_LLAMA_CPP_REF={llama_ref}");
    println!("cargo:rustc-env=SONA_CUDA_ADDON_SHERPA_ONNX_VERSION={sherpa_version}");
}
