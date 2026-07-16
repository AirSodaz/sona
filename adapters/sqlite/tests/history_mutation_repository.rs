use std::sync::Arc;

use sona_core::history::mutation_repository::{HistoryMutationError, HistoryMutationRepository};
use sona_core::history::mutation_service::HistoryMutationService;
use sona_core::history::{
    HistoryCreateLiveDraftRequest, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
};
use sona_sqlite::LazySqliteHistoryMutationRepository;

fn service(app_data_dir: &std::path::Path) -> HistoryMutationService {
    HistoryMutationService::new(Arc::new(LazySqliteHistoryMutationRepository::new(
        app_data_dir.to_path_buf(),
    )))
}

#[test]
fn lazy_repository_implements_the_core_history_mutation_port() {
    fn assert_port<T: HistoryMutationRepository>() {}

    assert_port::<LazySqliteHistoryMutationRepository>();
}

#[test]
fn core_validation_precedes_lazy_database_open() {
    let dir = tempfile::tempdir().unwrap();
    let error = service(dir.path())
        .create_live_draft(HistoryCreateLiveDraftRequest {
            id: None,
            audio_extension: "../wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        })
        .unwrap_err();

    assert!(matches!(error, HistoryMutationError::InvalidRequest(_)));
    assert_eq!(
        error.to_string(),
        "Invalid history mutation: audio extension must contain only 1-16 ASCII letters or digits"
    );
    assert!(!dir.path().join("sona.db").exists());
}

#[test]
fn missing_native_copy_source_is_rejected_before_database_open() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing-recording.wav");
    let error = service(dir.path())
        .save_recording(HistorySaveRecordingRequest {
            segments: Vec::new(),
            duration: 1.0,
            tag_ids: Vec::new(),
            audio_bytes: None,
            native_audio_path: Some(missing.to_string_lossy().into_owned()),
            audio_extension: Some("wav".to_string()),
        })
        .unwrap_err();

    assert!(matches!(error, HistoryMutationError::InvalidRequest(_)));
    assert!(
        error
            .to_string()
            .starts_with("Invalid history mutation: native recording source file")
    );
    assert!(!dir.path().join("sona.db").exists());
}

#[test]
fn missing_import_copy_source_is_rejected_before_database_open() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing-import.wav");
    let error = service(dir.path())
        .save_imported_file(HistorySaveImportedFileRequest {
            id: None,
            source_path: missing.to_string_lossy().into_owned(),
            segments: Vec::new(),
            duration: 1.0,
            tag_ids: Vec::new(),
            converted_source_path: None,
        })
        .unwrap_err();

    assert!(matches!(error, HistoryMutationError::InvalidRequest(_)));
    assert!(
        error
            .to_string()
            .starts_with("Invalid history mutation: import source file")
    );
    assert!(!dir.path().join("sona.db").exists());
}

#[test]
fn valid_mutation_opens_sqlite_and_persists_the_result() {
    let dir = tempfile::tempdir().unwrap();
    let result = service(dir.path())
        .create_live_draft(HistoryCreateLiveDraftRequest {
            id: Some("shared-adapter-draft".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        })
        .unwrap();

    assert_eq!(result.item.id, "shared-adapter-draft");
    assert!(dir.path().join("sona.db").is_file());
}
