use std::path::PathBuf;

use sona_core::models::catalog::{
    ModelListFilter, ModelSummary, list_models_with_installed_ids, select_models,
};

fn model_summary(id: &str, model_type: &str, language: &str, installed: bool) -> ModelSummary {
    ModelSummary {
        id: id.to_string(),
        name: format!("{id} name"),
        model_type: model_type.to_string(),
        language: language.to_string(),
        size: "1 MB".to_string(),
        modes: vec!["batch".to_string()],
        installed,
        install_path: PathBuf::from(format!("C:/models/{id}")),
    }
}

#[test]
fn lists_models_with_injected_install_status() {
    let models_dir = PathBuf::from("C:/models");

    let models = list_models_with_installed_ids(
        &models_dir,
        &std::collections::HashSet::from(["sherpa-onnx-whisper-turbo".to_string()]),
    );

    assert!(models.iter().any(|model| {
        model.id == "sherpa-onnx-whisper-turbo"
            && model.installed
            && model.install_path == models_dir.join("sherpa-onnx-whisper-turbo")
    }));
    assert!(models.iter().any(|model| {
        model.id == "silero-vad"
            && !model.installed
            && model.install_path == models_dir.join("silero_vad.onnx")
    }));
}

#[test]
fn selects_models_by_mode_type_language_and_install_status() {
    let models = vec![
        ModelSummary {
            modes: vec!["batch".to_string()],
            ..model_summary("whisper-zh", "whisper", "zh,en", true)
        },
        ModelSummary {
            modes: vec!["streaming".to_string()],
            ..model_summary("stream-zh", "zipformer", "zh", true)
        },
        ModelSummary {
            modes: vec!["batch".to_string()],
            ..model_summary("vad-all", "vad", "all", false)
        },
    ];

    let selected = select_models(
        models,
        &ModelListFilter {
            mode: Some("batch".to_string()),
            model_type: Some("whisper".to_string()),
            language: Some("zh".to_string()),
            installed_only: true,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "whisper-zh");
}
