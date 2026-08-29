use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sona_core::models::preset_models::{
    DEFAULT_MODEL_RULES, DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, LanguageMode,
    ModelCatalogSectionType, ModelDependencyConfigKey, ModelDependencyRequest, ModelSelectionPaths,
    build_model_catalog_snapshot_with_installed_ids, find_preset_model, preset_models,
};

#[test]
fn shared_preset_metadata_lives_in_core() {
    assert!(!preset_models().is_empty());
    assert!(find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).is_some());
    assert!(find_preset_model(DEFAULT_PUNCTUATION_MODEL_ID).is_some());
}

#[test]
fn qwen_gguf_presets_replace_the_sherpa_qwen_catalog_entry() {
    assert!(find_preset_model("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25").is_none());

    for (id, expected_model, expected_mmproj) in [
        (
            "qwen3-asr-0.6b-q8-gguf",
            "Qwen3-ASR-0.6B-Q8_0.gguf",
            "mmproj-Qwen3-ASR-0.6B-Q8_0.gguf",
        ),
        (
            "qwen3-asr-1.7b-q8-gguf",
            "Qwen3-ASR-1.7B-Q8_0.gguf",
            "mmproj-Qwen3-ASR-1.7B-Q8_0.gguf",
        ),
    ] {
        let model = find_preset_model(id).unwrap();
        assert_eq!(model.engine.as_deref(), Some("llama-cpp"));
        assert!(model.supports_mode("batch"));
        assert!(!model.supports_mode("streaming"));
        let rules = model.resolved_rules();
        assert!(rules.requires_vad);
        assert!(!rules.requires_punctuation);
        assert_eq!(model.artifacts.len(), 2);
        assert_eq!(
            model.file_config.as_ref().unwrap().model.as_deref(),
            Some(expected_model)
        );
        assert_eq!(
            model.file_config.as_ref().unwrap().mmproj.as_deref(),
            Some(expected_mmproj)
        );
        assert!(model.artifacts.iter().all(|artifact| {
            artifact.sha256.as_ref().is_some_and(|sha256| {
                sha256.len() == 64
                    && sha256
                        .chars()
                        .all(|character| character.is_ascii_hexdigit())
            })
        }));
    }
}
#[test]
fn parakeet_tdt_preset_is_a_verified_batch_bundle() {
    let model = find_preset_model("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8").unwrap();

    assert_eq!(model.model_type, "parakeet-tdt");
    assert_eq!(model.engine.as_deref(), Some("sherpa-onnx"));
    assert!(model.supports_mode("batch"));
    assert!(!model.supports_mode("streaming"));
    assert_eq!(model.language_mode, LanguageMode::Auto);
    assert_eq!(model.languages.len(), 25);

    let rules = model.resolved_rules();
    assert!(rules.requires_vad);
    assert!(!rules.requires_punctuation);

    let file_config = model.file_config.as_ref().unwrap();
    assert_eq!(file_config.encoder.as_deref(), Some("encoder.int8.onnx"));
    assert_eq!(file_config.decoder.as_deref(), Some("decoder.int8.onnx"));
    assert_eq!(file_config.joiner.as_deref(), Some("joiner.int8.onnx"));
    assert_eq!(file_config.tokens.as_deref(), Some("tokens.txt"));

    assert_eq!(model.artifacts.len(), 1);
    let artifact = &model.artifacts[0];
    assert_eq!(
        artifact.filename,
        "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2"
    );
    assert_eq!(
        artifact.sha256.as_deref(),
        Some("5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf")
    );
    assert_eq!(artifact.size_bytes, Some(487170055));
}
#[test]
fn paraformer_presets_are_verified_streaming_bundles() {
    for (id, expected_version, expected_encoder, expected_decoder) in [
        (
            "sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en-int8",
            "Int8",
            "encoder.int8.onnx",
            "decoder.int8.onnx",
        ),
        (
            "sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en",
            "Fp32",
            "encoder.onnx",
            "decoder.onnx",
        ),
    ] {
        let model = find_preset_model(id).unwrap();
        assert_eq!(model.model_type, "paraformer");
        assert_eq!(model.engine.as_deref(), Some("sherpa-onnx"));
        assert!(model.supports_mode("streaming"));
        assert!(!model.supports_mode("batch"));
        assert_eq!(model.language_mode, LanguageMode::Auto);
        assert_eq!(model.group_id.as_deref(), Some("paraformer"));
        assert_eq!(model.version_label.as_deref(), Some(expected_version));

        let rules = model.resolved_rules();
        assert!(!rules.requires_vad);
        assert!(rules.requires_punctuation);

        let file_config = model.file_config.as_ref().unwrap();
        assert_eq!(file_config.encoder.as_deref(), Some(expected_encoder));
        assert_eq!(file_config.decoder.as_deref(), Some(expected_decoder));
        assert_eq!(file_config.tokens.as_deref(), Some("tokens.txt"));
        assert_eq!(model.artifacts.len(), 3);
    }
}
#[test]
fn sensevoice_presets_are_verified_bundles() {
    for (id, expected_version, expected_model) in [
        (
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            "Int8",
            "model.int8.onnx",
        ),
        (
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
            "Fp32",
            "model.onnx",
        ),
    ] {
        let model = find_preset_model(id).unwrap();
        assert_eq!(model.model_type, "sensevoice");
        assert_eq!(model.engine.as_deref(), Some("sherpa-onnx"));
        assert!(model.supports_mode("streaming"));
        assert!(model.supports_mode("batch"));
        assert_eq!(model.language_mode, LanguageMode::Selectable);
        assert_eq!(model.group_id.as_deref(), Some("sensevoice"));
        assert_eq!(model.version_label.as_deref(), Some(expected_version));

        let rules = model.resolved_rules();
        assert!(rules.requires_vad);
        assert!(!rules.requires_punctuation);

        let file_config = model.file_config.as_ref().unwrap();
        assert_eq!(file_config.model.as_deref(), Some(expected_model));
        assert_eq!(file_config.tokens.as_deref(), Some("tokens.txt"));
        assert_eq!(model.artifacts.len(), 2);
    }
}

#[test]
fn all_preset_artifacts_have_valid_sha256_and_positive_size() {
    for model in preset_models() {
        assert!(
            !model.artifacts.is_empty(),
            "model '{}' should have at least one artifact",
            model.id
        );
        for artifact in &model.artifacts {
            assert!(
                artifact.size_bytes.is_some_and(|size| size > 0),
                "artifact '{}' in model '{}' should have positive sizeBytes",
                artifact.filename,
                model.id
            );
            assert!(
                artifact.sha256.as_ref().is_some_and(|sha256| {
                    sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit())
                }),
                "artifact '{}' in model '{}' should have a valid 64-char hex sha256",
                artifact.filename,
                model.id
            );
        }
    }
}

#[test]
fn resolves_model_paths_without_filesystem_status_checks() {
    let directory_model = find_preset_model("qwen3-asr-0.6b-q8-gguf").unwrap();
    assert_eq!(
        directory_model.resolve_install_path(Path::new("C:/models")),
        PathBuf::from("C:/models/qwen3-asr-0.6b-q8-gguf")
    );

    let file_model = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    assert_eq!(
        file_model.resolve_install_path(Path::new("C:/models")),
        PathBuf::from("C:/models/silero_vad_v5.onnx")
    );
    assert_eq!(
        file_model.resolve_download_path(Path::new("C:/models")),
        PathBuf::from("C:/models/silero_vad_v5.onnx")
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
    assert!(silero.install_path.ends_with("silero_vad_v5.onnx"));

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

    let selected = sona_core::models::preset_models::resolve_model_catalog_selected_ids(
        &snapshot,
        &ModelSelectionPaths {
            streaming_model_path: int8_path,
            batch_model_path: "D:\\portable\\qwen3-asr-0.6b-q8-gguf".to_string(),
            speaker_segmentation_model_path: String::new(),
            speaker_embedding_model_path:
                "D:/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx".to_string(),
        },
    );

    assert_eq!(
        selected.streaming,
        Some("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string())
    );
    assert_eq!(selected.batch, Some("qwen3-asr-0.6b-q8-gguf".to_string()));
    assert_eq!(selected.speaker_segmentation, None);
    assert_eq!(
        selected.speaker_embedding,
        Some("3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx".to_string())
    );
}

/// Language metadata invariants shared by every surface (GUI, CLI, FFI).
///
/// The preset JSON is the single source of truth for language pickers, so it
/// must stay normalized: no legacy `multi` markers, no duplicates, ascending
/// ISO 639 codes, and modes that agree with the list shape.
#[test]
fn language_metadata_is_normalized_across_all_presets() {
    for model in preset_models() {
        assert!(
            model
                .languages
                .iter()
                .all(|code| !code.eq_ignore_ascii_case("multi")),
            "{} must not use the legacy multi marker",
            model.id
        );
        let mut sorted = model.languages.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            model.languages, sorted,
            "{} languages must be sorted+unique",
            model.id
        );
        for code in &model.languages {
            assert!(
                (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase()),
                "{} has invalid language code {code}",
                model.id
            );
        }

        match model.language_mode {
            LanguageMode::Selectable => assert!(
                model.languages.len() >= 2,
                "{} selectable models should offer multiple languages",
                model.id
            ),
            LanguageMode::Auto => assert!(
                model.languages.len() >= 2,
                "{} auto models are multilingual by definition",
                model.id
            ),
            LanguageMode::Fixed => assert_eq!(
                model.languages.len(),
                1,
                "{} fixed models must declare exactly one language",
                model.id
            ),
            LanguageMode::None => assert!(
                model.languages.is_empty(),
                "{} non-ASR models must not declare languages",
                model.id
            ),
        }
    }
}

#[test]
fn asr_language_modes_match_engine_capabilities() {
    let selectable = |id: &str| find_preset_model(id).unwrap().language_mode;
    let mode = |id: &str| find_preset_model(id).unwrap().language_mode;

    // Engines accepting a language parameter.
    assert_eq!(
        selectable("sherpa-onnx-whisper-large-v3"),
        LanguageMode::Selectable
    );
    assert_eq!(
        mode("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"),
        LanguageMode::Selectable
    );
    assert_eq!(
        mode("sherpa-onnx-funasr-nano-int8-2025-12-30"),
        LanguageMode::Selectable
    );

    // Engines ignoring or rejecting language overrides.
    assert_eq!(mode("qwen3-asr-0.6b-q8-gguf"), LanguageMode::Auto);
    assert_eq!(
        mode("sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en"),
        LanguageMode::Auto
    );
    assert_eq!(
        mode("sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en-int8"),
        LanguageMode::Auto
    );
    assert_eq!(
        mode("sherpa-onnx-dolphin-small-ctc-multi-lang-int8-2025-04-02"),
        LanguageMode::Auto
    );
    assert_eq!(
        mode("sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26"),
        LanguageMode::Auto
    );
    assert_eq!(
        mode("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"),
        LanguageMode::Auto
    );

    // Single-language engines.
    assert_eq!(
        mode("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30"),
        LanguageMode::Fixed
    );

    // Companion models carry no language semantics anymore.
    assert_eq!(mode("silero-vad"), LanguageMode::None);
    assert_eq!(
        mode("sherpa-onnx-pyannote-segmentation-3-0"),
        LanguageMode::None
    );
}

#[test]
fn supports_language_follows_mode_rules() {
    let whisper = find_preset_model("sherpa-onnx-whisper-large-v3").unwrap();
    assert!(whisper.supports_language("auto"));
    assert!(whisper.supports_language("ja"));
    assert!(!whisper.supports_language("xx"));

    let zipformer =
        find_preset_model("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30").unwrap();
    assert!(zipformer.supports_language("auto"));
    assert!(zipformer.supports_language("zh"));
    assert!(!zipformer.supports_language("en"));

    let vad = find_preset_model("silero-vad").unwrap();
    assert!(!vad.supports_language("auto"));

    let v5_vad = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    assert_eq!(v5_vad.id, "silero-v5-vad");
    assert!(!v5_vad.supports_language("auto"));
}
