use sona_core::models::downloads::{required_companion_models, resolve_model_download};
use sona_core::models::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use sona_core::runtime::error::RuntimeValidationError;

#[test]
fn resolves_model_download_paths_and_required_companions() {
    let models_dir = std::path::Path::new("C:/models");
    let resolved =
        resolve_model_download("sherpa-onnx-funasr-nano-int8-2025-12-30", models_dir).unwrap();

    assert_eq!(
        resolved.install_path,
        models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30")
    );
    assert_eq!(
        resolved.download_path,
        models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30")
    );

    let companions = required_companion_models(&resolved.model);
    assert_eq!(
        companions.vad_model_id.as_deref(),
        Some(DEFAULT_SILERO_VAD_MODEL_ID)
    );
    assert_eq!(
        companions.punctuation_model_id.as_deref(),
        Some(DEFAULT_PUNCTUATION_MODEL_ID)
    );
}

#[test]
fn resolves_qwen_gguf_artifacts_inside_atomic_install_directory() {
    let models_dir = std::path::Path::new("C:/models");
    let resolved = resolve_model_download("qwen3-asr-0.6b-q8-gguf", models_dir).unwrap();

    assert_eq!(
        resolved.install_path,
        models_dir.join("qwen3-asr-0.6b-q8-gguf")
    );
    assert_eq!(resolved.artifacts.len(), 2);
    assert_eq!(
        resolved.artifacts[0].install_path,
        resolved.install_path.join("Qwen3-ASR-0.6B-Q8_0.gguf")
    );
    assert_eq!(
        resolved.artifacts[1].install_path,
        resolved
            .install_path
            .join("mmproj-Qwen3-ASR-0.6B-Q8_0.gguf")
    );
}

#[test]
fn unknown_model_download_preserves_model_id_validation_context() {
    let error =
        resolve_model_download("missing-model", std::path::Path::new("C:/models")).unwrap_err();

    assert_eq!(
        error,
        RuntimeValidationError::new("model_id", "Unknown model id: missing-model")
    );
}
