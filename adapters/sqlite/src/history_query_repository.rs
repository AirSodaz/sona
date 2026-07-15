use std::path::PathBuf;
use std::sync::Arc;

use sona_core::history::query_repository::{HistoryQueryError, HistoryQueryRepository};
use sona_core::history::{
    HistoryItemRecord, HistoryListOptions, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, TranscriptSnapshotMetadata, TranscriptSnapshotRecord,
};
use sona_core::transcription::transcript::TranscriptSegment;

use crate::{Database, history_store::SqliteHistoryStore};

#[derive(Clone, Debug)]
pub struct LazySqliteHistoryQueryRepository {
    app_local_data_dir: PathBuf,
}

impl LazySqliteHistoryQueryRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn with_store<T>(
        &self,
        operation: impl FnOnce(&SqliteHistoryStore) -> Result<T, HistoryQueryError>,
    ) -> Result<T, HistoryQueryError> {
        let database = Database::open(&self.app_local_data_dir)
            .map_err(|error| HistoryQueryError::Database(error.to_string()))?;
        let store = SqliteHistoryStore::new(self.app_local_data_dir.clone(), Arc::new(database));
        operation(&store)
    }
}

impl HistoryQueryRepository for LazySqliteHistoryQueryRepository {
    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, HistoryQueryError> {
        self.with_store(|store| HistoryQueryRepository::list_items(store))
    }

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError> {
        self.with_store(|store| {
            HistoryQueryRepository::list_items_with_reconciled_live_drafts(store)
        })
    }

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError> {
        self.with_store(|store| HistoryQueryRepository::list_items_paginated(store, opts))
    }

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError> {
        self.with_store(|store| {
            HistoryQueryRepository::list_items_with_reconciled_live_drafts_paginated(store, opts)
        })
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryQueryError> {
        self.with_store(|store| HistoryQueryRepository::query_workspace(store, request))
    }

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryQueryError> {
        self.with_store(|store| HistoryQueryRepository::load_transcript(store, history_id))
    }

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryQueryError> {
        self.with_store(|store| {
            HistoryQueryRepository::list_transcript_snapshots(store, history_id)
        })
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryQueryError> {
        self.with_store(|store| {
            HistoryQueryRepository::load_transcript_snapshot(store, history_id, snapshot_id)
        })
    }
}
