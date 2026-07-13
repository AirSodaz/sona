use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryMutationError, HistoryMutationRepository,
    HistoryReassignProjectRequest, HistoryUpdateItemMetaRequest,
    HistoryUpdateProjectAssignmentsRequest, HistoryUpdateTranscriptRequest,
};
use sona_core::history::mutation_service::HistoryMutationService;
use sona_core::history::{
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
};
use sona_sqlite::{Database, SqliteHistoryStore};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistorySaveRecordingMetadata {
    segments: Value,
    duration: f64,
    project_id: Option<String>,
    audio_extension: Option<String>,
}

struct LazySqliteHistoryMutationRepository {
    app_data_dir: PathBuf,
}

impl LazySqliteHistoryMutationRepository {
    fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    fn with_store<T>(
        &self,
        operation: impl FnOnce(&SqliteHistoryStore) -> Result<T, HistoryMutationError>,
    ) -> Result<T, HistoryMutationError> {
        let database = Database::open(&self.app_data_dir).map_err(HistoryMutationError::from)?;
        let store = SqliteHistoryStore::new(self.app_data_dir.clone(), Arc::new(database));
        operation(&store)
    }
}

impl HistoryMutationRepository for LazySqliteHistoryMutationRepository {
    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::create_live_draft(store, request))
    }

    fn complete_live_draft(
        &self,
        request: HistoryCompleteLiveDraftRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::complete_live_draft(store, request))
    }

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        if let Some(native_audio_path) = request.native_audio_path.as_deref() {
            validate_copy_source("native recording source file", native_audio_path)?;
        }
        self.with_store(|store| HistoryMutationRepository::save_recording(store, request))
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        let copy_source = request
            .converted_source_path
            .as_deref()
            .unwrap_or(&request.source_path);
        validate_copy_source("import source file", copy_source)?;
        self.with_store(|store| HistoryMutationRepository::save_imported_file(store, request))
    }

    fn delete_items(&self, request: HistoryDeleteItemsRequest) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::delete_items(store, request))
    }

    fn update_transcript(
        &self,
        request: HistoryUpdateTranscriptRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::update_transcript(store, request))
    }

    fn create_transcript_snapshot(
        &self,
        request: HistoryCreateTranscriptSnapshotRequest,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError> {
        self.with_store(|store| {
            HistoryMutationRepository::create_transcript_snapshot(store, request)
        })
    }

    fn update_item_meta(
        &self,
        request: HistoryUpdateItemMetaRequest,
    ) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::update_item_meta(store, request))
    }

    fn update_project_assignments(
        &self,
        request: HistoryUpdateProjectAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        self.with_store(|store| {
            HistoryMutationRepository::update_project_assignments(store, request)
        })
    }

    fn reassign_project(
        &self,
        request: HistoryReassignProjectRequest,
    ) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::reassign_project(store, request))
    }
}

fn validate_copy_source(label: &str, path: &str) -> Result<(), HistoryMutationError> {
    if Path::new(path).is_file() {
        Ok(())
    } else {
        Err(HistoryMutationError::InvalidRequest(format!(
            "{label} does not exist or is not a file: {path}"
        )))
    }
}

pub(crate) async fn create_history_live_draft_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.create_live_draft(request)
    })
    .await
}

pub(crate) async fn complete_history_live_draft_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.complete_live_draft(request)
    })
    .await
}

pub(crate) async fn save_history_recording_json(
    app_data_dir: String,
    request_json: String,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
) -> SonaCoreBindingResult<String> {
    let metadata: HistorySaveRecordingMetadata = parse_request(&request_json)?;
    let request = HistorySaveRecordingRequest {
        segments: metadata.segments,
        duration: metadata.duration,
        project_id: metadata.project_id,
        audio_bytes,
        native_audio_path,
        audio_extension: metadata.audio_extension,
    };
    run_mutation(app_data_dir, move |service| service.save_recording(request)).await
}

pub(crate) async fn save_history_imported_file_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.save_imported_file(request)
    })
    .await
}

pub(crate) async fn delete_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| service.delete_items(request)).await
}

pub(crate) async fn update_history_transcript_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.update_transcript(request)
    })
    .await
}

pub(crate) async fn create_history_transcript_snapshot_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.create_transcript_snapshot(request)
    })
    .await
}

pub(crate) async fn update_history_item_meta_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.update_item_meta(request)
    })
    .await
}

pub(crate) async fn update_history_project_assignments_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.update_project_assignments(request)
    })
    .await
}

pub(crate) async fn reassign_history_project_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.reassign_project(request)
    })
    .await
}

fn parse_request<T: DeserializeOwned>(request_json: &str) -> SonaCoreBindingResult<T> {
    serde_json::from_str(request_json)
        .map_err(HistoryMutationError::Serialization)
        .map_err(history_mutation_error)
}

async fn run_mutation<T, F>(app_data_dir: String, operation: F) -> SonaCoreBindingResult<String>
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let app_data_dir =
            std::path::absolute(PathBuf::from(app_data_dir)).map_err(history_mutation_error)?;
        ensure_existing_directory(&app_data_dir)?;
        let repository = Arc::new(LazySqliteHistoryMutationRepository::new(app_data_dir));
        operation(HistoryMutationService::new(repository))
            .map_err(history_mutation_error)
            .and_then(canonical_json)
    })
    .await
    .map_err(history_mutation_error)?
}

fn ensure_existing_directory(path: &Path) -> SonaCoreBindingResult<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(history_mutation_error(format!(
            "History app data directory does not exist: {}",
            path.display()
        )))
    }
}

fn canonical_json(value: impl serde::Serialize) -> SonaCoreBindingResult<String> {
    let canonical = serde_json::to_value(value)
        .map_err(HistoryMutationError::Serialization)
        .map_err(history_mutation_error)?;
    serde_json::to_string(&canonical)
        .map_err(HistoryMutationError::Serialization)
        .map_err(history_mutation_error)
}

fn history_mutation_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::HistoryMutation {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        complete_history_live_draft_json, create_history_live_draft_json,
        create_history_transcript_snapshot_json, delete_history_items_json,
        reassign_history_project_json, save_history_imported_file_json,
        save_history_recording_json, update_history_item_meta_json,
        update_history_project_assignments_json, update_history_transcript_json,
    };
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};

    fn app_data_dir(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    fn canonical(output: String) -> Value {
        let value: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), output);
        value
    }

    #[tokio::test]
    async fn malformed_json_is_rejected_before_app_data_or_database_access() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");

        let error =
            create_history_live_draft_json(missing.to_string_lossy().into_owned(), "{".to_string())
                .await
                .unwrap_err();

        assert!(matches!(
            error,
            SonaCoreBindingError::HistoryMutation { .. }
        ));
        assert!(error.to_string().starts_with("Serialization error:"));
        assert!(!missing.exists());
        assert!(!root.path().join("sona.db").exists());
    }

    #[tokio::test]
    async fn core_validation_runs_before_the_lazy_repository_opens_sqlite() {
        let dir = tempfile::tempdir().unwrap();

        let error = create_history_live_draft_json(
            app_data_dir(&dir),
            json!({"id": null, "audioExtension": "../wav", "projectId": null, "icon": null})
                .to_string(),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            SonaCoreBindingError::HistoryMutation { .. }
        ));
        assert_eq!(
            error.to_string(),
            "Invalid history mutation: audio extension must contain only 1-16 ASCII letters or digits"
        );
        assert!(!dir.path().join("sona.db").exists());
    }

    #[tokio::test]
    async fn missing_native_recording_source_is_invalid_before_sqlite_is_opened() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing-recording.wav");

        let error = save_history_recording_json(
            app_data_dir(&dir),
            json!({
                "segments": [],
                "duration": 1.0,
                "projectId": null,
                "audioExtension": "wav"
            })
            .to_string(),
            None,
            Some(missing.to_string_lossy().into_owned()),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            SonaCoreBindingError::HistoryMutation { .. }
        ));
        assert!(
            error
                .to_string()
                .starts_with("Invalid history mutation: native recording source file")
        );
        assert!(!dir.path().join("sona.db").exists());
    }

    #[tokio::test]
    async fn missing_import_copy_source_is_invalid_before_sqlite_is_opened() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing-import.wav");

        let error = save_history_imported_file_json(
            app_data_dir(&dir),
            json!({
                "id": null,
                "sourcePath": missing.to_string_lossy(),
                "segments": [],
                "duration": 1.0,
                "projectId": null,
                "convertedSourcePath": null
            })
            .to_string(),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            SonaCoreBindingError::HistoryMutation { .. }
        ));
        assert!(
            error
                .to_string()
                .starts_with("Invalid history mutation: import source file")
        );
        assert!(!dir.path().join("sona.db").exists());
    }

    #[tokio::test]
    async fn all_history_mutations_return_canonical_json_and_unit_as_null() {
        let dir = tempfile::tempdir().unwrap();
        let app_data = app_data_dir(&dir);
        let segments = json!([{"id":"s1","text":"hello","start":0.0,"end":1.0,"isFinal":true}]);

        let draft = canonical(
            create_history_live_draft_json(
                app_data.clone(),
                json!({"id":"live-1","audioExtension":"wav","projectId":null,"icon":null})
                    .to_string(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(draft["item"]["id"], "live-1");

        let completed = canonical(
            complete_history_live_draft_json(
                app_data.clone(),
                json!({"historyId":"live-1","segments":segments,"duration":1.0}).to_string(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(completed["id"], "live-1");

        let recording = canonical(
            save_history_recording_json(
                app_data.clone(),
                json!({"segments":segments,"duration":1.0,"projectId":null,"audioExtension":"wav"})
                    .to_string(),
                Some(vec![1, 2, 3]),
                None,
            )
            .await
            .unwrap(),
        );
        let recording_id = recording["id"].as_str().unwrap().to_string();

        let source = dir.path().join("import.wav");
        std::fs::write(&source, [4_u8, 5, 6]).unwrap();
        let imported = canonical(
            save_history_imported_file_json(
                app_data.clone(),
                json!({
                    "id":"import-1",
                    "sourcePath":source.to_string_lossy(),
                    "segments":segments,
                    "duration":1.0,
                    "projectId":null,
                    "convertedSourcePath":null
                })
                .to_string(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(imported["id"], "import-1");

        let transcript = canonical(
            update_history_transcript_json(
                app_data.clone(),
                json!({"historyId":recording_id,"segments":segments}).to_string(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(transcript["id"], recording_id);

        let snapshot = canonical(
            create_history_transcript_snapshot_json(
                app_data.clone(),
                json!({"historyId":recording_id,"reason":"polish","segments":segments}).to_string(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(snapshot["historyId"], recording_id);

        let units = [
            update_history_item_meta_json(
                app_data.clone(),
                json!({"historyId":recording_id,"updates":{"title":"Mobile title"}}).to_string(),
            )
            .await
            .unwrap(),
            update_history_project_assignments_json(
                app_data.clone(),
                json!({"ids":[recording_id],"projectId":null}).to_string(),
            )
            .await
            .unwrap(),
            reassign_history_project_json(
                app_data.clone(),
                json!({"currentProjectId":"unused","nextProjectId":null}).to_string(),
            )
            .await
            .unwrap(),
            delete_history_items_json(app_data, json!({"ids":["import-1"]}).to_string())
                .await
                .unwrap(),
        ];
        assert!(units.into_iter().all(|output| output == "null"));
    }
}
