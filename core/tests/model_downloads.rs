use sona_core::models::downloads::{required_companion_models, resolve_model_download};
use sona_core::models::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};

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
        models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30.tar.bz2")
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
