use std::path::{Path, PathBuf};

use sona_core::model_catalog::{
    ModelListFilter, ModelSummary, list_models, remove_model_install_path, select_models,
};

fn model_summary(id: &str, model_type: &str, language: &str, installed: bool) -> ModelSummary {
    ModelSummary {
        id: id.to_string(),
        name: format!("{id} name"),
        model_type: model_type.to_string(),
        language: language.to_string(),
        size: "1 MB".to_string(),
        modes: vec!["offline".to_string()],
        installed,
        install_path: PathBuf::from(format!("C:/models/{id}")),
    }
}

#[test]
fn lists_models_with_install_status_from_models_dir() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

    let models = list_models(&models_dir);

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
            modes: vec!["offline".to_string()],
            ..model_summary("whisper-zh", "whisper", "zh,en", true)
        },
        ModelSummary {
            modes: vec!["streaming".to_string()],
            ..model_summary("stream-zh", "zipformer", "zh", true)
        },
        ModelSummary {
            modes: vec!["offline".to_string()],
            ..model_summary("vad-all", "vad", "all", false)
        },
    ];

    let selected = select_models(
        models,
        &ModelListFilter {
            mode: Some("offline".to_string()),
            model_type: Some("whisper".to_string()),
            language: Some("zh".to_string()),
            installed_only: true,
        },
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "whisper-zh");
}

#[test]
fn removes_file_and_directory_install_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("silero_vad.onnx");
    let directory_path = dir.path().join("sherpa-onnx-whisper-turbo");

    std::fs::write(&file_path, "fake").unwrap();
    std::fs::create_dir_all(&directory_path).unwrap();
    std::fs::write(directory_path.join("model.onnx"), "fake").unwrap();

    remove_model_install_path(&file_path).unwrap();
    remove_model_install_path(&directory_path).unwrap();

    assert!(!file_path.exists());
    assert!(!directory_path.exists());
}

#[test]
fn removing_missing_install_path_is_a_noop() {
    let missing = Path::new("C:/definitely/not/present/model.onnx");

    remove_model_install_path(missing).unwrap();
}
