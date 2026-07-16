use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryMutationError, HistoryReplaceTagAssignmentsRequest,
    HistoryTrashItemsRequest, HistoryUpdateTagAssignmentsRequest, HistoryUpdateTranscriptRequest,
};
use sona_core::history::mutation_service::HistoryMutationService;
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history::transcript_payload::normalize_history_transcript_segments;
use sona_core::history::{
    HistorySaveImportedFileRequest, HistorySaveRecordingRequest, HistoryWorkspaceDateFilter,
    HistoryWorkspaceFilterType, HistoryWorkspaceQueryRequest, HistoryWorkspaceScope,
    HistoryWorkspaceSortOrder, TranscriptSnapshotReason,
};
use sona_sqlite::{LazySqliteHistoryMutationRepository, LazySqliteHistoryQueryRepository};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistorySaveRecordingMetadata {
    segments: Value,
    duration: f64,
    #[serde(default)]
    tag_ids: Vec<String>,
    #[serde(default)]
    project_id: Option<String>,
    audio_extension: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryCompleteLiveDraftJsonRequest {
    history_id: String,
    segments: Value,
    duration: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistorySaveImportedFileJsonRequest {
    id: Option<String>,
    source_path: String,
    segments: Value,
    duration: f64,
    #[serde(default)]
    tag_ids: Vec<String>,
    #[serde(default)]
    project_id: Option<String>,
    converted_source_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryUpdateTranscriptJsonRequest {
    history_id: String,
    segments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoryCreateTranscriptSnapshotJsonRequest {
    history_id: String,
    reason: TranscriptSnapshotReason,
    segments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyHistoryProjectAssignmentsRequest {
    ids: Vec<String>,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyHistoryReassignProjectRequest {
    current_project_id: String,
    next_project_id: Option<String>,
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
    let request: HistoryCompleteLiveDraftJsonRequest = parse_request(&request_json)?;
    let request = HistoryCompleteLiveDraftRequest {
        history_id: request.history_id,
        segments: parse_legacy_segments(request.segments)?,
        duration: request.duration,
    };
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
        segments: parse_legacy_segments(metadata.segments)?,
        duration: metadata.duration,
        tag_ids: normalized_tag_ids(metadata.tag_ids, metadata.project_id),
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
    let request: HistorySaveImportedFileJsonRequest = parse_request(&request_json)?;
    let request = HistorySaveImportedFileRequest {
        id: request.id,
        source_path: request.source_path,
        segments: parse_legacy_segments(request.segments)?,
        duration: request.duration,
        tag_ids: normalized_tag_ids(request.tag_ids, request.project_id),
        converted_source_path: request.converted_source_path,
    };
    run_mutation(app_data_dir, move |service| {
        service.save_imported_file(request)
    })
    .await
}

pub(crate) async fn delete_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryDeleteItemsRequest = parse_request(&request_json)?;
    let deleted_at = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(history_mutation_error)?
            .as_millis(),
    )
    .map_err(history_mutation_error)?;
    run_mutation(app_data_dir, move |service| {
        service.trash_items(HistoryTrashItemsRequest {
            ids: request.ids,
            deleted_at,
        })
    })
    .await
}

pub(crate) async fn trash_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryTrashItemsRequest = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| service.trash_items(request)).await
}

pub(crate) async fn restore_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryDeleteItemsRequest = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| service.restore_items(request)).await
}

pub(crate) async fn purge_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryDeleteItemsRequest = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| service.purge_items(request)).await
}

pub(crate) async fn update_history_transcript_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryUpdateTranscriptJsonRequest = parse_request(&request_json)?;
    let request = HistoryUpdateTranscriptRequest {
        history_id: request.history_id,
        segments: parse_legacy_segments(request.segments)?,
    };
    run_mutation(app_data_dir, move |service| {
        service.update_transcript(request)
    })
    .await
}

pub(crate) async fn create_history_transcript_snapshot_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryCreateTranscriptSnapshotJsonRequest = parse_request(&request_json)?;
    let request = HistoryCreateTranscriptSnapshotRequest {
        history_id: request.history_id,
        reason: request.reason,
        segments: parse_legacy_segments(request.segments)?,
    };
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
    let request: LegacyHistoryProjectAssignmentsRequest = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.replace_tag_assignments(HistoryReplaceTagAssignmentsRequest {
            ids: request.ids,
            tag_ids: request.project_id.into_iter().collect(),
        })
    })
    .await
}

pub(crate) async fn reassign_history_project_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: LegacyHistoryReassignProjectRequest = parse_request(&request_json)?;
    reassign_history_tag_compat(app_data_dir, request).await
}

pub(crate) async fn update_history_tag_assignments_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryUpdateTagAssignmentsRequest = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.update_tag_assignments(request)
    })
    .await
}

pub(crate) async fn replace_history_tag_assignments_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: HistoryReplaceTagAssignmentsRequest = parse_request(&request_json)?;
    run_mutation(app_data_dir, move |service| {
        service.replace_tag_assignments(request)
    })
    .await
}

async fn reassign_history_tag_compat(
    app_data_dir: String,
    request: LegacyHistoryReassignProjectRequest,
) -> SonaCoreBindingResult<String> {
    tokio::task::spawn_blocking(move || {
        let app_data_dir =
            std::path::absolute(PathBuf::from(app_data_dir)).map_err(history_mutation_error)?;
        ensure_existing_directory(&app_data_dir)?;
        let query = HistoryQueryService::new(Arc::new(LazySqliteHistoryQueryRepository::new(
            app_data_dir.clone(),
        )));
        let mut ids = Vec::new();
        for scope in [HistoryWorkspaceScope::All, HistoryWorkspaceScope::Trash] {
            let mut offset = 0;
            loop {
                let page = query
                    .query_workspace(HistoryWorkspaceQueryRequest {
                        scope: scope.clone(),
                        query: String::new(),
                        filter_type: HistoryWorkspaceFilterType::All,
                        date_filter: HistoryWorkspaceDateFilter::All,
                        sort_order: HistoryWorkspaceSortOrder::Newest,
                        limit: 200,
                        offset,
                    })
                    .map_err(history_mutation_error)?;
                ids.extend(
                    page.filtered_items
                        .iter()
                        .filter(|item| item.tag_ids.contains(&request.current_project_id))
                        .map(|item| item.id.clone()),
                );
                if !page.has_more {
                    break;
                }
                offset += page.filtered_items.len();
            }
        }
        let mutation = HistoryMutationService::new(Arc::new(
            LazySqliteHistoryMutationRepository::new(app_data_dir),
        ));
        mutation
            .update_tag_assignments(HistoryUpdateTagAssignmentsRequest {
                ids,
                add_tag_ids: request.next_project_id.into_iter().collect(),
                remove_tag_ids: vec![request.current_project_id],
            })
            .map_err(history_mutation_error)?;
        canonical_json(())
    })
    .await
    .map_err(history_mutation_error)?
}

fn normalized_tag_ids(tag_ids: Vec<String>, project_id: Option<String>) -> Vec<String> {
    if tag_ids.is_empty() {
        project_id.into_iter().collect()
    } else {
        tag_ids
    }
}

fn parse_request<T: DeserializeOwned>(request_json: &str) -> SonaCoreBindingResult<T> {
    serde_json::from_str(request_json)
        .map_err(HistoryMutationError::Serialization)
        .map_err(history_mutation_error)
}

fn parse_legacy_segments(
    segments: Value,
) -> SonaCoreBindingResult<Vec<sona_core::transcription::transcript::TranscriptSegment>> {
    normalize_history_transcript_segments(segments)
        .map(|normalized| normalized.segments)
        .map_err(HistoryMutationError::InvalidRequest)
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
    use crate::history_query_bridge::load_history_transcript_json;
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
    async fn legacy_json_segments_are_normalized_at_the_uniffi_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let app_data = app_data_dir(&dir);

        create_history_live_draft_json(
            app_data.clone(),
            json!({
                "id": "legacy-live",
                "audioExtension": "wav",
                "projectId": null,
                "icon": null
            })
            .to_string(),
        )
        .await
        .unwrap();
        complete_history_live_draft_json(
            app_data.clone(),
            json!({
                "historyId": "legacy-live",
                "segments": [{"text": "legacy transcript"}],
                "duration": 1.0
            })
            .to_string(),
        )
        .await
        .unwrap();

        let transcript = canonical(
            load_history_transcript_json(app_data, "legacy-live".to_string())
                .await
                .unwrap(),
        );
        assert_eq!(transcript[0]["id"], "segment-0");
        assert_eq!(transcript[0]["isFinal"], true);
        assert_eq!(transcript[0]["timing"]["source"], "derived");
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
