use std::sync::Arc;

use crate::history::query_repository::{HistoryQueryError, HistoryQueryRepository};
use crate::history::workspace_query::validate_workspace_query_request;
use crate::history::{
    HistoryItemRecord, HistoryListOptions, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, TranscriptSnapshotMetadata, TranscriptSnapshotRecord,
};
use crate::transcription::transcript::TranscriptSegment;

pub struct HistoryQueryService {
    repository: Arc<dyn HistoryQueryRepository>,
}

impl HistoryQueryService {
    pub fn new(repository: Arc<dyn HistoryQueryRepository>) -> Self {
        Self { repository }
    }

    pub fn list_items(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError> {
        validate_pagination_value("limit", opts.limit)?;
        validate_pagination_value("offset", opts.offset)?;
        self.repository
            .list_items_with_reconciled_live_drafts_paginated(opts)
    }

    pub fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryQueryError> {
        validate_workspace_query_request(&request)?;
        self.repository.query_workspace(request)
    }

    pub fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryQueryError> {
        validate_id("history ID", history_id)?;
        self.repository.load_transcript(history_id)
    }

    pub fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryQueryError> {
        validate_id("history ID", history_id)?;
        self.repository.list_transcript_snapshots(history_id)
    }

    pub fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryQueryError> {
        validate_id("history ID", history_id)?;
        validate_id("snapshot ID", snapshot_id)?;
        self.repository
            .load_transcript_snapshot(history_id, snapshot_id)
    }
}

fn validate_pagination_value(label: &str, value: Option<usize>) -> Result<(), HistoryQueryError> {
    if value.is_some_and(|value| u64::try_from(value).unwrap_or(u64::MAX) > i64::MAX as u64) {
        Err(HistoryQueryError::InvalidRequest(format!(
            "{label} exceeds the supported range"
        )))
    } else {
        Ok(())
    }
}

fn validate_id(label: &str, value: &str) -> Result<(), HistoryQueryError> {
    if value.trim().is_empty() {
        Err(HistoryQueryError::InvalidRequest(format!(
            "{label} must not be empty"
        )))
    } else {
        Ok(())
    }
}
