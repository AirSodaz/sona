use std::path::PathBuf;

use sona_core::models::catalog::{
    ModelListFilter, ModelSummary, list_models_with_installed_ids, select_models,
};
use sona_core::models::preset_models::LanguageMode;

fn model_summary(id: &str, languages: &[&str], installed: bool) -> ModelSummary {
    ModelSummary {
        id: id.to_string(),
        name: format!("{id} name"),
        model_type: "whisper".to_string(),
        languages: languages.iter().map(|code| code.to_string()).collect(),
        language_mode: LanguageMode::Selectable,
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
            ..model_summary("whisper-zh", &["en", "zh"], true)
        },
        ModelSummary {
            modes: vec!["streaming".to_string()],
            model_type: "zipformer".to_string(),
            language_mode: LanguageMode::Fixed,
            languages: vec!["zh".to_string()],
            installed: true,
            id: "stream-zh".to_string(),
            name: "stream-zh name".to_string(),
            size: "1 MB".to_string(),
            install_path: PathBuf::from("C:/models/stream-zh"),
        },
        ModelSummary {
            modes: vec!["batch".to_string()],
            model_type: "vad".to_string(),
            language_mode: LanguageMode::None,
            languages: vec![],
            installed: false,
            id: "vad-none".to_string(),
            name: "vad-none name".to_string(),
            size: "1 MB".to_string(),
            install_path: PathBuf::from("C:/models/vad-none"),
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
