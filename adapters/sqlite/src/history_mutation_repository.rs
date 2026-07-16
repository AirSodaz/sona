use std::path::{Path, PathBuf};
use std::sync::Arc;

use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest, HistoryMutationError,
    HistoryMutationRepository, HistoryPurgeItemsRequest, HistoryReplaceTagAssignmentsRequest,
    HistoryRestoreItemsRequest, HistoryTrashItemsRequest, HistoryUpdateItemMetaRequest,
    HistoryUpdateTagAssignmentsRequest, HistoryUpdateTranscriptRequest,
};
use sona_core::history::{
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
};

use crate::{Database, history_store::SqliteHistoryStore};

#[derive(Clone, Debug)]
pub struct LazySqliteHistoryMutationRepository {
    app_local_data_dir: PathBuf,
}

impl LazySqliteHistoryMutationRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn with_store<T>(
        &self,
        operation: impl FnOnce(&SqliteHistoryStore) -> Result<T, HistoryMutationError>,
    ) -> Result<T, HistoryMutationError> {
        let database =
            Database::open(&self.app_local_data_dir).map_err(HistoryMutationError::from)?;
        let store = SqliteHistoryStore::new(self.app_local_data_dir.clone(), Arc::new(database));
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

    fn trash_items(&self, request: HistoryTrashItemsRequest) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::trash_items(store, request))
    }

    fn restore_items(
        &self,
        request: HistoryRestoreItemsRequest,
    ) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::restore_items(store, request))
    }

    fn purge_items(&self, request: HistoryPurgeItemsRequest) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::purge_items(store, request))
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

    fn update_tag_assignments(
        &self,
        request: HistoryUpdateTagAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::update_tag_assignments(store, request))
    }

    fn replace_tag_assignments(
        &self,
        request: HistoryReplaceTagAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        self.with_store(|store| HistoryMutationRepository::replace_tag_assignments(store, request))
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
