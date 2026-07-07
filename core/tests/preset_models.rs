use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use sona_core::preset_models::{
    DEFAULT_MODEL_RULES, DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID,
    ModelCatalogSectionType, ModelDependencyConfigKey, ModelDependencyRequest, ModelSelectionPaths,
    build_model_catalog_snapshot, build_model_catalog_snapshot_with_installed_ids,
    find_preset_model, is_preset_model_installed_at, preset_models,
};

#[test]
fn shared_preset_metadata_lives_in_core() {
    assert!(!preset_models().is_empty());
    assert!(find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).is_some());
    assert!(find_preset_model(DEFAULT_PUNCTUATION_MODEL_ID).is_some());
}

#[test]
fn resolves_model_paths_without_filesystem_status_checks() {
    let directory_model = find_preset_model("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25").unwrap();
    assert_eq!(
        directory_model.resolve_install_path(Path::new("C:/models")),
        PathBuf::from("C:/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")
    );

    let file_model = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    assert_eq!(
        file_model.resolve_install_path(Path::new("C:/models")),
        PathBuf::from("C:/models/silero_vad.onnx")
    );
    assert_eq!(
        file_model.resolve_download_path(Path::new("C:/models")),
        PathBuf::from("C:/models/silero_vad.onnx")
    );

    let archive_model = find_preset_model("sherpa-onnx-whisper-turbo").unwrap();
    assert_eq!(
        archive_model.resolve_download_path(Path::new("C:/models")),
        PathBuf::from("C:/models/sherpa-onnx-whisper-turbo.tar.bz2")
    );
}

#[test]
fn preset_rules_and_modes_are_core_domain_metadata() {
    let vad = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    assert_eq!(vad.resolved_rules(), DEFAULT_MODEL_RULES);
    assert!(!vad.supports_mode("batch"));

    let offline = find_preset_model("sherpa-onnx-whisper-turbo").unwrap();
    assert!(offline.supports_mode("batch"));
}

#[test]
fn builds_catalog_snapshot_from_injected_install_status() {
    let installed_ids = HashSet::from([
        "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
        DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
    ]);

    let snapshot =
        build_model_catalog_snapshot_with_installed_ids(Path::new("C:/models"), &installed_ids);

    assert_eq!(snapshot.models_dir, "C:/models");
    let silero = snapshot
        .models
        .iter()
        .find(|model| model.id == DEFAULT_SILERO_VAD_MODEL_ID)
        .unwrap();
    assert!(silero.is_installed);
    assert!(silero.install_path.ends_with("silero_vad.onnx"));

    let asr_section = snapshot
        .sections
        .iter()
        .find(|section| section.section_type == ModelCatalogSectionType::Asr)
        .unwrap();
    let sensevoice_group = asr_section
        .groups
        .iter()
        .find(|group| group.key == "sensevoice")
        .unwrap();
    assert_eq!(
        sensevoice_group
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
        ]
    );

    let int8_path = snapshot
        .model_path_by_id
        .get("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
        .unwrap()
        .clone();
    assert_eq!(
        snapshot.restore_defaults.streaming_model_path,
        Some(int8_path.clone())
    );
    assert_eq!(snapshot.restore_defaults.batch_model_path, Some(int8_path));
    assert_eq!(
        snapshot.restore_defaults.vad_model_path,
        snapshot
            .model_path_by_id
            .get(DEFAULT_SILERO_VAD_MODEL_ID)
            .cloned()
    );

    let sense_voice_dependencies = snapshot
        .dependency_requests_by_model_id
        .get("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
        .unwrap();
    assert_eq!(
        sense_voice_dependencies,
        &vec![ModelDependencyRequest {
            model_id: DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            config_key: ModelDependencyConfigKey::VadModelPath,
            install_path: snapshot.model_path_by_id[DEFAULT_SILERO_VAD_MODEL_ID].clone(),
            is_installed: true,
        }]
    );
}

#[test]
fn build_model_catalog_snapshot_uses_filesystem_install_status() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(models_dir.join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"))
        .unwrap();
    fs::write(
        models_dir.join("silero_vad.onnx"),
        b"not the expected model",
    )
    .unwrap();

    let snapshot = build_model_catalog_snapshot(&models_dir);
    let int8_path = snapshot
        .model_path_by_id
        .get("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
        .unwrap()
        .clone();
    let silero_path = snapshot
        .model_path_by_id
        .get(DEFAULT_SILERO_VAD_MODEL_ID)
        .unwrap()
        .clone();

    assert_eq!(
        snapshot.models_dir,
        models_dir.to_string_lossy().to_string()
    );
    assert!(snapshot.models.iter().any(|model| {
        model.id == "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17" && model.is_installed
    }));
    assert!(
        snapshot
            .models
            .iter()
            .any(|model| model.id == DEFAULT_SILERO_VAD_MODEL_ID && model.is_installed)
    );
    assert_eq!(
        snapshot.restore_defaults.streaming_model_path,
        Some(int8_path.clone())
    );
    assert_eq!(snapshot.restore_defaults.batch_model_path, Some(int8_path));
    assert_eq!(snapshot.restore_defaults.vad_model_path, Some(silero_path));
}

#[test]
fn non_archive_install_status_requires_non_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path();
    let empty_path = models_dir.join("silero_vad.onnx");
    fs::write(&empty_path, []).unwrap();
    let model = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();

    assert!(!is_preset_model_installed_at(model, models_dir));

    fs::write(&empty_path, b"model bytes").unwrap();
    assert!(is_preset_model_installed_at(model, models_dir));

    fs::remove_file(&empty_path).unwrap();
    fs::create_dir_all(&empty_path).unwrap();
    assert!(!is_preset_model_installed_at(model, models_dir));
}

#[test]
fn archive_install_status_accepts_existing_install_path() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model = find_preset_model("sherpa-onnx-whisper-turbo").unwrap();
    fs::create_dir_all(model.resolve_install_path(&models_dir)).unwrap();

    assert!(is_preset_model_installed_at(model, &models_dir));
}

#[test]
fn resolves_catalog_selection_ids_without_adapter_state() {
    let snapshot = build_model_catalog_snapshot_with_installed_ids(
        Path::new("C:/models"),
        &HashSet::from([
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
            "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx".to_string(),
        ]),
    );
    let int8_path = snapshot
        .model_path_by_id
        .get("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
        .unwrap()
        .clone();

    let selected = sona_core::preset_models::resolve_model_catalog_selected_ids(
        &snapshot,
        &ModelSelectionPaths {
            streaming_model_path: int8_path,
            batch_model_path: "D:\\portable\\sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25"
                .to_string(),
            speaker_segmentation_model_path: String::new(),
            speaker_embedding_model_path:
                "D:/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx".to_string(),
        },
    );

    assert_eq!(
        selected.streaming,
        Some("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string())
    );
    assert_eq!(
        selected.batch,
        Some("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25".to_string())
    );
    assert_eq!(selected.speaker_segmentation, None);
    assert_eq!(
        selected.speaker_embedding,
        Some("3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx".to_string())
    );
}
