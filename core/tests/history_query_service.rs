use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use sona_core::history::query_repository::HistoryQueryRepository;
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history::{
    HistoryListOptions, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult,
    HistoryWorkspaceScope, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    TranscriptSnapshotMetadata, TranscriptSnapshotRecord,
};
use sona_core::history_store::HistoryStoreError;
use sona_core::transcription::transcript::TranscriptSegment;

#[derive(Default)]
struct RecordingHistoryQueryRepository {
    calls: Mutex<Vec<String>>,
}

impl HistoryQueryRepository for RecordingHistoryQueryRepository {
    fn list_items(&self) -> Result<Vec<sona_core::history::HistoryItemRecord>, HistoryStoreError> {
        Ok(Vec::new())
    }

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<sona_core::history::HistoryItemRecord>, HistoryStoreError> {
        Ok(Vec::new())
    }

    fn list_items_paginated(
        &self,
        _opts: HistoryListOptions,
    ) -> Result<Vec<sona_core::history::HistoryItemRecord>, HistoryStoreError> {
        Ok(Vec::new())
    }

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<sona_core::history::HistoryItemRecord>, HistoryStoreError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("list:{:?}:{:?}", opts.limit, opts.offset));
        Ok(Vec::new())
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryStoreError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("query:{}:{}", request.limit, request.offset));
        Ok(empty_workspace_result())
    }

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryStoreError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("transcript:{history_id}"));
        Ok(None)
    }

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryStoreError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("snapshots:{history_id}"));
        Ok(Vec::new())
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryStoreError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("snapshot:{history_id}:{snapshot_id}"));
        Ok(None)
    }
}

fn empty_workspace_result() -> HistoryWorkspaceQueryResult {
    HistoryWorkspaceQueryResult {
        filtered_items: Vec::new(),
        search_match_by_item_id: BTreeMap::new(),
        filtered_item_count: 0,
        has_more: false,
        summary: HistoryWorkspaceSummary {
            total_items: 0,
            total_duration: 0.0,
            latest_timestamp: None,
            recording_count: 0,
            batch_count: 0,
        },
        item_counts: HistoryWorkspaceItemCounts {
            untagged: 0,
            trash: 0,
            by_tag_id: BTreeMap::new(),
        },
    }
}

fn workspace_request() -> HistoryWorkspaceQueryRequest {
    HistoryWorkspaceQueryRequest {
        scope: HistoryWorkspaceScope::All,
        query: String::new(),
        filter_type: HistoryWorkspaceFilterType::All,
        date_filter: HistoryWorkspaceDateFilter::All,
        sort_order: HistoryWorkspaceSortOrder::Newest,
        limit: 25,
        offset: 5,
    }
}

#[test]
fn service_routes_all_history_query_operations_through_the_focused_port() {
    let repository = Arc::new(RecordingHistoryQueryRepository::default());
    let service = HistoryQueryService::new(repository.clone());

    service
        .list_items(HistoryListOptions {
            limit: Some(20),
            offset: Some(3),
        })
        .unwrap();
    service.query_workspace(workspace_request()).unwrap();
    service.load_transcript("history-1").unwrap();
    service.list_transcript_snapshots("history-1").unwrap();
    service
        .load_transcript_snapshot("history-1", "snapshot-1")
        .unwrap();

    assert_eq!(
        *repository.calls.lock().unwrap(),
        [
            "list:Some(20):Some(3)",
            "query:25:5",
            "transcript:history-1",
            "snapshots:history-1",
            "snapshot:history-1:snapshot-1",
        ]
    );
}

#[test]
fn service_rejects_empty_ids_before_calling_the_repository() {
    let repository = Arc::new(RecordingHistoryQueryRepository::default());
    let service = HistoryQueryService::new(repository.clone());

    for error in [
        service.load_transcript("  ").unwrap_err(),
        service.list_transcript_snapshots("").unwrap_err(),
        service
            .load_transcript_snapshot("history-1", " \t")
            .unwrap_err(),
    ] {
        assert!(matches!(error, HistoryStoreError::InvalidRequest(_)));
    }
    assert!(repository.calls.lock().unwrap().is_empty());
}

#[test]
fn service_rejects_pagination_values_outside_the_portable_storage_range() {
    if usize::BITS <= 63 {
        return;
    }
    let repository = Arc::new(RecordingHistoryQueryRepository::default());
    let service = HistoryQueryService::new(repository.clone());

    for opts in [
        HistoryListOptions {
            limit: Some(usize::MAX),
            offset: None,
        },
        HistoryListOptions {
            limit: None,
            offset: Some(usize::MAX),
        },
    ] {
        assert!(matches!(
            service.list_items(opts).unwrap_err(),
            HistoryStoreError::InvalidRequest(_)
        ));
    }
    assert!(repository.calls.lock().unwrap().is_empty());
}
