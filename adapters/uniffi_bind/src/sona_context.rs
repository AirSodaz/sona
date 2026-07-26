use std::sync::Arc;

use crate::application_context::{ContextSource, HostApplicationContext};
use crate::mapper::*;
use crate::{
    SonaCoreBindingError, SonaCoreBindingResult, app_config_repository_bridge, automation_bridge,
    backup_bridge, dashboard_bridge, diagnostics_bridge, history_mutation_bridge,
    history_query_bridge, recovery_bridge, storage_usage_bridge, sync_bridge, tag_bridge,
    task_ledger_bridge,
};

/// An explicit composition root for one application-data directory.
///
/// The free functions in `lib.rs` take a directory string and resolve it
/// through the process-wide registry on every call. `SonaContext` resolves once
/// at construction and holds the result, so every operation on the handle uses
/// that context directly — no registry lookup, no lock, and no chance that a
/// typo in a path silently opens a second database.
///
/// Holding the handle also pins the entry: the registry never evicts a context
/// a caller still owns, so the handle's context stays the one context for its
/// directory for as long as it lives.
///
/// Both surfaces run the same bridge code. Bridges take a `ContextSource`, and
/// this type simply supplies the owned variant rather than a path, so there is
/// no second implementation to keep in step.
#[derive(uniffi::Object)]
pub struct SonaContext {
    inner: Arc<HostApplicationContext>,
}

#[uniffi::export]
impl SonaContext {
    /// Opens (or joins) the context for `app_data_dir` and holds it.
    #[uniffi::constructor]
    pub fn open(app_data_dir: String) -> SonaCoreBindingResult<Arc<Self>> {
        let inner = ContextSource::from(app_data_dir)
            .resolve()
            .map_err(|error| SonaCoreBindingError::InvalidInput {
                reason: error.to_string(),
            })?;
        Ok(Arc::new(Self { inner }))
    }

    /// The normalized application-data directory this context owns.
    ///
    /// This is the registry's canonical form, so it may differ from the string
    /// passed to `open` — path aliases resolve to one directory.
    pub fn app_data_dir(&self) -> String {
        self.inner
            .sqlite()
            .app_data_dir()
            .to_string_lossy()
            .into_owned()
    }
}

impl SonaContext {
    /// The context source to hand a bridge. Not exported: callers get typed
    /// operations, never the context internals.
    pub(crate) fn source(&self) -> ContextSource {
        ContextSource::from(&self.inner)
    }
}

// ---------------------------------------------------------------------
// Domain operations. Generated from the exported free functions so the two
// surfaces cannot drift; each hands the bridge this context instead of a
// directory to look up.
#[uniffi::export(async_runtime = "tokio")]
impl SonaContext {
    pub fn load_tag_repository_state_json(&self) -> SonaCoreBindingResult<String> {
        tag_bridge::load_tag_repository_state_json(self.source())
    }

    pub fn load_tag_repository_v1(&self) -> SonaCoreBindingResult<FfiTagRepositorySnapshotV1> {
        tag_bridge::load_tag_repository_v1(self.source())
    }

    pub fn replace_tags_json(&self, tags_json: String) -> SonaCoreBindingResult<()> {
        tag_bridge::replace_tags_json(self.source(), tags_json)
    }

    pub fn replace_tags_v1(&self, tags: Vec<FfiTagRecordV1>) -> SonaCoreBindingResult<()> {
        tag_bridge::replace_tags_v1(self.source(), tags)
    }

    pub fn create_tag_json(&self, input_json: String) -> SonaCoreBindingResult<String> {
        tag_bridge::create_tag_json(self.source(), input_json)
    }

    pub fn create_tag_v1(
        &self,
        input: FfiTagCreateInputV1,
    ) -> SonaCoreBindingResult<FfiTagRecordV1> {
        tag_bridge::create_tag_v1(self.source(), input)
    }

    pub fn update_tag_json(
        &self,
        tag_id: String,
        updates_json: String,
    ) -> SonaCoreBindingResult<String> {
        tag_bridge::update_tag_json(self.source(), tag_id, updates_json)
    }

    pub fn update_tag_v1(
        &self,
        tag_id: String,
        updates: FfiTagUpdateInputV1,
    ) -> SonaCoreBindingResult<Option<FfiTagRecordV1>> {
        tag_bridge::update_tag_v1(self.source(), tag_id, updates)
    }

    pub fn delete_tag(&self, tag_id: String) -> SonaCoreBindingResult<()> {
        tag_bridge::delete_tag(self.source(), tag_id)
    }

    pub fn delete_tag_v1(&self, tag_id: String) -> SonaCoreBindingResult<()> {
        tag_bridge::delete_tag_v1(self.source(), tag_id)
    }

    pub fn reorder_tags_json(&self, tag_ids_json: String) -> SonaCoreBindingResult<String> {
        tag_bridge::reorder_tags_json(self.source(), tag_ids_json)
    }

    pub fn reorder_tags_v1(
        &self,
        tag_ids: Vec<String>,
    ) -> SonaCoreBindingResult<Vec<FfiTagRecordV1>> {
        tag_bridge::reorder_tags_v1(self.source(), tag_ids)
    }

    pub fn set_active_tag_id(&self, tag_id: Option<String>) -> SonaCoreBindingResult<()> {
        tag_bridge::set_active_tag_id(self.source(), tag_id)
    }

    pub fn set_active_tag_id_v1(&self, tag_id: Option<String>) -> SonaCoreBindingResult<()> {
        tag_bridge::set_active_tag_id_v1(self.source(), tag_id)
    }

    pub fn load_recovery_snapshot_json(&self) -> SonaCoreBindingResult<String> {
        recovery_bridge::load_recovery_snapshot_json(self.app_data_dir())
    }

    pub fn load_recovery_snapshot_v1(&self) -> SonaCoreBindingResult<FfiRecoverySnapshotV1> {
        recovery_bridge::load_recovery_snapshot_v1(self.app_data_dir())
    }

    pub fn save_recovery_snapshot_json(&self, items_json: String) -> SonaCoreBindingResult<String> {
        recovery_bridge::save_recovery_snapshot_json(self.app_data_dir(), items_json)
    }

    pub fn save_recovery_snapshot_v1(
        &self,
        items: Vec<FfiRecoveryItemInputV1>,
    ) -> SonaCoreBindingResult<FfiRecoverySnapshotV1> {
        recovery_bridge::save_recovery_snapshot_v1(self.app_data_dir(), items)
    }

    pub fn persist_recovery_queue_snapshot_json(
        &self,
        queue_items_json: String,
        resolved_ids: Vec<String>,
    ) -> SonaCoreBindingResult<String> {
        recovery_bridge::persist_recovery_queue_snapshot_json(
            self.app_data_dir(),
            queue_items_json,
            resolved_ids,
        )
    }

    pub fn persist_recovery_queue_snapshot_v1(
        &self,
        queue_items: Vec<FfiRecoveryItemInputV1>,
        resolved_ids: Vec<String>,
    ) -> SonaCoreBindingResult<FfiRecoverySnapshotV1> {
        recovery_bridge::persist_recovery_queue_snapshot_v1(
            self.app_data_dir(),
            queue_items,
            resolved_ids,
        )
    }

    pub fn load_task_ledger_snapshot_json(&self) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::load_task_ledger_snapshot_json(self.source())
    }

    pub fn load_task_ledger_snapshot_v1(&self) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
        task_ledger_bridge::load_task_ledger_snapshot_v1(self.source())
    }

    pub fn upsert_task_ledger_record_json(
        &self,
        record_json: String,
    ) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::upsert_task_ledger_record_json(self.source(), record_json)
    }

    pub fn upsert_task_ledger_record_v1(
        &self,
        record: FfiTaskLedgerRecordV1,
    ) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
        task_ledger_bridge::upsert_task_ledger_record_v1(self.source(), record)
    }

    pub fn patch_task_ledger_record_json(
        &self,
        id: String,
        patch_json: String,
    ) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::patch_task_ledger_record_json(self.source(), id, patch_json)
    }

    pub fn patch_task_ledger_record_v1(
        &self,
        id: String,
        patch: FfiTaskLedgerPatchV1,
    ) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
        task_ledger_bridge::patch_task_ledger_record_v1(self.source(), id, patch)
    }

    pub fn remove_task_ledger_record_json(&self, id: String) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::remove_task_ledger_record_json(self.source(), id)
    }

    pub fn remove_task_ledger_record_v1(
        &self,
        id: String,
    ) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
        task_ledger_bridge::remove_task_ledger_record_v1(self.source(), id)
    }

    pub fn clear_resolved_task_ledger_records_json(&self) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::clear_resolved_task_ledger_records_json(self.source())
    }

    pub fn clear_resolved_task_ledger_records_v1(
        &self,
    ) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
        task_ledger_bridge::clear_resolved_task_ledger_records_v1(self.source())
    }

    pub fn load_automation_repository_state_json(&self) -> SonaCoreBindingResult<String> {
        automation_bridge::load_automation_repository_state_json(self.source())
    }

    pub fn load_automation_repository_state_v1(
        &self,
    ) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
        automation_bridge::load_automation_repository_state_v1(self.source())
    }

    pub fn replace_automation_rules_json(
        &self,
        rules_json: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::replace_automation_rules_json(self.source(), rules_json)
    }

    pub fn replace_automation_rules_v1(
        &self,
        rules: Vec<FfiAutomationRuleInputV1>,
    ) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
        automation_bridge::replace_automation_rules_v1(self.source(), rules)
    }

    pub fn replace_automation_processed_entries_json(
        &self,
        entries_json: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::replace_automation_processed_entries_json(self.source(), entries_json)
    }

    pub fn replace_automation_processed_entries_v1(
        &self,
        entries: Vec<FfiAutomationProcessedInputV1>,
    ) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
        automation_bridge::replace_automation_processed_entries_v1(self.source(), entries)
    }

    pub fn replace_automation_repository_state_json(
        &self,
        state_json: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::replace_automation_repository_state_json(self.source(), state_json)
    }

    pub fn replace_automation_repository_state_v1(
        &self,
        input: FfiAutomationRepositoryInputV1,
    ) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
        automation_bridge::replace_automation_repository_state_v1(self.source(), input)
    }

    pub async fn export_backup_archive_json(
        &self,
        archive_path: String,
        app_version: String,
    ) -> SonaCoreBindingResult<String> {
        backup_bridge::export_backup_archive_json(self.source(), archive_path, app_version).await
    }

    pub async fn import_backup_archive_json(
        &self,
        archive_path: String,
        default_rule_set_name: String,
        confirm_replace: bool,
    ) -> SonaCoreBindingResult<String> {
        backup_bridge::import_backup_archive_json(
            self.source(),
            archive_path,
            default_rule_set_name,
            confirm_replace,
        )
        .await
    }

    pub async fn export_backup_archive_v1(
        &self,
        archive_path: String,
        app_version: String,
    ) -> SonaCoreBindingResult<FfiBackupManifestV1> {
        backup_bridge::export_backup_archive_v1(self.source(), archive_path, app_version).await
    }

    pub async fn import_backup_archive_v1(
        &self,
        archive_path: String,
        default_rule_set_name: String,
        confirm_replace: bool,
    ) -> SonaCoreBindingResult<FfiBackupApplyResultV1> {
        backup_bridge::import_backup_archive_v1(
            self.source(),
            archive_path,
            default_rule_set_name,
            confirm_replace,
        )
        .await
    }

    pub async fn sync_get_status_json(&self) -> SonaCoreBindingResult<String> {
        sync_bridge::get_status_json(self.source()).await
    }

    pub async fn sync_create_vault_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        sync_bridge::create_vault_json(self.source(), request_json).await
    }

    pub async fn sync_preview_join_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        sync_bridge::preview_join_json(self.source(), request_json).await
    }

    pub async fn sync_join_vault_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        sync_bridge::join_vault_json(self.source(), request_json).await
    }

    pub async fn sync_unlock_json(&self, request_json: String) -> SonaCoreBindingResult<String> {
        sync_bridge::unlock_json(self.source(), request_json, false).await
    }

    pub async fn sync_unlock_with_recovery_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        sync_bridge::unlock_json(self.source(), request_json, true).await
    }

    pub async fn sync_lock(&self) -> SonaCoreBindingResult<()> {
        sync_bridge::lock(self.source()).await
    }

    pub async fn sync_set_paused_json(&self, paused: bool) -> SonaCoreBindingResult<String> {
        sync_bridge::set_paused_json(self.source(), paused).await
    }

    pub async fn sync_disconnect_json(&self) -> SonaCoreBindingResult<String> {
        sync_bridge::disconnect_json(self.source()).await
    }

    pub async fn sync_run_now_json(&self) -> SonaCoreBindingResult<String> {
        sync_bridge::run_now_json(self.source()).await
    }

    pub async fn sync_change_preset_json(
        &self,
        preset_json: String,
        confirm_shrink: bool,
    ) -> SonaCoreBindingResult<String> {
        sync_bridge::change_preset_json(self.source(), preset_json, confirm_shrink).await
    }

    pub async fn sync_change_master_password_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<()> {
        sync_bridge::change_master_password_json(self.source(), request_json).await
    }

    pub async fn sync_generate_recovery_key(&self) -> SonaCoreBindingResult<String> {
        sync_bridge::generate_recovery_key(self.source()).await
    }

    pub fn sync_list_conflicts_json(&self) -> SonaCoreBindingResult<String> {
        sync_bridge::list_conflicts_json(self.source())
    }

    pub fn sync_get_conflict_json(&self, conflict_id: String) -> SonaCoreBindingResult<String> {
        sync_bridge::get_conflict_json(self.source(), conflict_id)
    }

    pub fn sync_resolve_conflict_json(
        &self,
        conflict_id: String,
        resolution_json: String,
    ) -> SonaCoreBindingResult<()> {
        sync_bridge::resolve_conflict_json(self.source(), conflict_id, resolution_json)
    }

    pub async fn sync_get_status_v1(&self) -> SonaCoreBindingResult<FfiSyncStatusSnapshotV1> {
        sync_bridge::get_status_v1(self.source()).await
    }

    pub async fn sync_create_vault_v1(
        &self,
        request: FfiSyncCreateRequestV1,
    ) -> SonaCoreBindingResult<FfiSyncCreateResultV1> {
        sync_bridge::create_vault_v1(self.source(), request).await
    }

    pub async fn sync_preview_join_v1(
        &self,
        request: FfiSyncJoinRequestV1,
    ) -> SonaCoreBindingResult<FfiSyncJoinPreviewV1> {
        sync_bridge::preview_join_v1(self.source(), request).await
    }

    pub async fn sync_join_vault_v1(
        &self,
        request: FfiSyncJoinRequestV1,
    ) -> SonaCoreBindingResult<FfiSyncRunResultV1> {
        sync_bridge::join_vault_v1(self.source(), request).await
    }

    pub async fn sync_unlock_v1(
        &self,
        request: FfiSyncUnlockRequestV1,
    ) -> SonaCoreBindingResult<FfiSyncStatusSnapshotV1> {
        sync_bridge::unlock_v1(self.source(), request, false).await
    }

    pub async fn sync_unlock_with_recovery_v1(
        &self,
        request: FfiSyncUnlockRequestV1,
    ) -> SonaCoreBindingResult<FfiSyncStatusSnapshotV1> {
        sync_bridge::unlock_v1(self.source(), request, true).await
    }

    pub async fn sync_set_paused_v1(
        &self,
        paused: bool,
    ) -> SonaCoreBindingResult<FfiSyncStatusSnapshotV1> {
        sync_bridge::set_paused_v1(self.source(), paused).await
    }

    pub async fn sync_disconnect_v1(&self) -> SonaCoreBindingResult<FfiSyncStatusSnapshotV1> {
        sync_bridge::disconnect_v1(self.source()).await
    }

    pub async fn sync_run_now_v1(&self) -> SonaCoreBindingResult<FfiSyncRunResultV1> {
        sync_bridge::run_now_v1(self.source()).await
    }

    pub async fn sync_change_preset_v1(
        &self,
        preset: FfiSyncPresetV1,
        confirm_shrink: bool,
    ) -> SonaCoreBindingResult<FfiSyncStatusSnapshotV1> {
        sync_bridge::change_preset_v1(self.source(), preset, confirm_shrink).await
    }

    pub async fn sync_change_master_password_v1(
        &self,
        request: FfiSyncChangePasswordRequestV1,
    ) -> SonaCoreBindingResult<()> {
        sync_bridge::change_master_password_v1(self.source(), request).await
    }

    pub fn sync_list_conflicts_v1(&self) -> SonaCoreBindingResult<Vec<FfiSyncConflictSummaryV1>> {
        sync_bridge::list_conflicts_v1(self.source())
    }

    pub fn sync_get_conflict_v1(
        &self,
        conflict_id: String,
    ) -> SonaCoreBindingResult<Option<FfiSyncConflictDetailV1>> {
        sync_bridge::get_conflict_v1(self.source(), conflict_id)
    }

    pub fn sync_resolve_conflict_v1(
        &self,
        conflict_id: String,
        resolution: FfiSyncConflictResolutionV1,
    ) -> SonaCoreBindingResult<()> {
        sync_bridge::resolve_conflict_v1(self.source(), conflict_id, resolution)
    }

    pub async fn list_history_items_json(
        &self,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::list_history_items_json(self.source(), limit, offset).await
    }

    pub async fn list_history_items_v1(
        &self,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> SonaCoreBindingResult<Vec<FfiHistoryItemRecordV1>> {
        history_query_bridge::list_history_items_v1(self.source(), limit, offset).await
    }

    pub async fn query_history_workspace_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::query_history_workspace_json(self.source(), request_json).await
    }

    pub async fn query_history_workspace_v1(
        &self,
        request: FfiHistoryWorkspaceQueryRequestV1,
    ) -> SonaCoreBindingResult<FfiHistoryWorkspaceQueryResultV1> {
        history_query_bridge::query_history_workspace_v1(self.source(), request).await
    }

    pub async fn load_history_transcript_json(
        &self,
        history_id: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::load_history_transcript_json(self.source(), history_id).await
    }

    pub async fn load_history_transcript_v1(
        &self,
        history_id: String,
    ) -> SonaCoreBindingResult<Option<Vec<FfiTranscriptSegment>>> {
        history_query_bridge::load_history_transcript_v1(self.source(), history_id).await
    }

    pub async fn list_history_transcript_snapshots_json(
        &self,
        history_id: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::list_history_transcript_snapshots_json(self.source(), history_id)
            .await
    }

    pub async fn list_history_transcript_snapshots_v1(
        &self,
        history_id: String,
    ) -> SonaCoreBindingResult<Vec<FfiTranscriptSnapshotMetadataV1>> {
        history_query_bridge::list_history_transcript_snapshots_v1(self.source(), history_id).await
    }

    pub async fn load_history_transcript_snapshot_json(
        &self,
        history_id: String,
        snapshot_id: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::load_history_transcript_snapshot_json(
            self.source(),
            history_id,
            snapshot_id,
        )
        .await
    }

    pub async fn load_history_transcript_snapshot_v1(
        &self,
        history_id: String,
        snapshot_id: String,
    ) -> SonaCoreBindingResult<Option<FfiTranscriptSnapshotRecordV1>> {
        history_query_bridge::load_history_transcript_snapshot_v1(
            self.source(),
            history_id,
            snapshot_id,
        )
        .await
    }

    pub async fn create_history_live_draft_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::create_history_live_draft_json(self.source(), request_json).await
    }

    pub async fn create_history_live_draft_v1(
        &self,
        request: FfiHistoryCreateLiveDraftRequestV1,
    ) -> SonaCoreBindingResult<FfiLiveRecordingDraftResultV1> {
        history_mutation_bridge::create_history_live_draft_v1(self.source(), request).await
    }

    pub async fn complete_history_live_draft_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::complete_history_live_draft_json(self.source(), request_json).await
    }

    pub async fn complete_history_live_draft_v1(
        &self,
        request: FfiHistoryCompleteLiveDraftRequestV1,
    ) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
        history_mutation_bridge::complete_history_live_draft_v1(self.source(), request).await
    }

    pub async fn save_history_recording_json(
        &self,
        request_json: String,
        audio_bytes: Option<Vec<u8>>,
        native_audio_path: Option<String>,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::save_history_recording_json(
            self.source(),
            request_json,
            audio_bytes,
            native_audio_path,
        )
        .await
    }

    pub async fn save_history_recording_v1(
        &self,
        request: FfiHistorySaveRecordingRequestV1,
    ) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
        history_mutation_bridge::save_history_recording_v1(self.source(), request).await
    }

    pub async fn save_history_imported_file_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::save_history_imported_file_json(self.source(), request_json).await
    }

    pub async fn save_history_imported_file_v1(
        &self,
        request: FfiHistorySaveImportedFileRequestV1,
    ) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
        history_mutation_bridge::save_history_imported_file_v1(self.source(), request).await
    }

    pub async fn delete_history_items_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::delete_history_items_json(self.source(), request_json).await
    }

    pub async fn trash_history_items_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::trash_history_items_json(self.source(), request_json).await
    }

    pub async fn trash_history_items_v1(
        &self,
        request: FfiHistoryTrashItemsRequestV1,
    ) -> SonaCoreBindingResult<()> {
        history_mutation_bridge::trash_history_items_v1(self.source(), request).await
    }

    pub async fn restore_history_items_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::restore_history_items_json(self.source(), request_json).await
    }

    pub async fn restore_history_items_v1(
        &self,
        request: FfiHistoryDeleteItemsRequestV1,
    ) -> SonaCoreBindingResult<()> {
        history_mutation_bridge::restore_history_items_v1(self.source(), request).await
    }

    pub async fn purge_history_items_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::purge_history_items_json(self.source(), request_json).await
    }

    pub async fn purge_history_items_v1(
        &self,
        request: FfiHistoryDeleteItemsRequestV1,
    ) -> SonaCoreBindingResult<()> {
        history_mutation_bridge::purge_history_items_v1(self.source(), request).await
    }

    pub async fn update_history_transcript_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_transcript_json(self.source(), request_json).await
    }

    pub async fn update_history_transcript_v1(
        &self,
        request: FfiHistoryUpdateTranscriptRequestV1,
    ) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
        history_mutation_bridge::update_history_transcript_v1(self.source(), request).await
    }

    pub async fn create_history_transcript_snapshot_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::create_history_transcript_snapshot_json(
            self.source(),
            request_json,
        )
        .await
    }

    pub async fn create_history_transcript_snapshot_v1(
        &self,
        request: FfiHistoryCreateTranscriptSnapshotRequestV1,
    ) -> SonaCoreBindingResult<FfiTranscriptSnapshotMetadataV1> {
        history_mutation_bridge::create_history_transcript_snapshot_v1(self.source(), request).await
    }

    pub async fn update_history_item_meta_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_item_meta_json(self.source(), request_json).await
    }

    pub async fn update_history_item_meta_v1(
        &self,
        request: FfiHistoryUpdateItemMetaRequestV1,
    ) -> SonaCoreBindingResult<()> {
        history_mutation_bridge::update_history_item_meta_v1(self.source(), request).await
    }

    pub async fn update_history_project_assignments_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_project_assignments_json(
            self.source(),
            request_json,
        )
        .await
    }

    pub async fn update_history_tag_assignments_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_tag_assignments_json(self.source(), request_json)
            .await
    }

    pub async fn update_history_tag_assignments_v1(
        &self,
        request: FfiHistoryUpdateTagAssignmentsRequestV1,
    ) -> SonaCoreBindingResult<()> {
        history_mutation_bridge::update_history_tag_assignments_v1(self.source(), request).await
    }

    pub async fn replace_history_tag_assignments_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::replace_history_tag_assignments_json(self.source(), request_json)
            .await
    }

    pub async fn replace_history_tag_assignments_v1(
        &self,
        request: FfiHistoryReplaceTagAssignmentsRequestV1,
    ) -> SonaCoreBindingResult<()> {
        history_mutation_bridge::replace_history_tag_assignments_v1(self.source(), request).await
    }

    pub async fn reassign_history_project_json(
        &self,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::reassign_history_project_json(self.source(), request_json).await
    }

    pub fn load_app_config_json(&self) -> SonaCoreBindingResult<Option<String>> {
        app_config_repository_bridge::load_app_config_json(self.source())
    }

    pub fn save_app_config_json(&self, config_json: String) -> SonaCoreBindingResult<()> {
        app_config_repository_bridge::save_app_config_json(self.source(), config_json)
    }

    pub fn get_app_setting_json(&self, key: String) -> SonaCoreBindingResult<Option<String>> {
        app_config_repository_bridge::get_app_setting_json(self.source(), key)
    }

    pub fn set_app_setting_json(
        &self,
        key: String,
        value_json: String,
    ) -> SonaCoreBindingResult<()> {
        app_config_repository_bridge::set_app_setting_json(self.source(), key, value_json)
    }

    pub async fn load_dashboard_snapshot_json(&self, deep: bool) -> SonaCoreBindingResult<String> {
        dashboard_bridge::load_dashboard_snapshot_json(self.source(), deep).await
    }

    pub async fn load_dashboard_snapshot_v1(
        &self,
        deep: bool,
    ) -> SonaCoreBindingResult<FfiDashboardSnapshotV1> {
        dashboard_bridge::load_dashboard_snapshot_v1(self.source(), deep).await
    }

    pub async fn load_diagnostics_snapshot_json(
        &self,
        input_json: String,
    ) -> SonaCoreBindingResult<String> {
        diagnostics_bridge::load_diagnostics_snapshot_json(self.app_data_dir(), input_json).await
    }

    pub async fn load_diagnostics_snapshot_v1(
        &self,
        input: FfiDiagnosticsInputV1,
    ) -> SonaCoreBindingResult<FfiDiagnosticsSnapshotV1> {
        diagnostics_bridge::load_diagnostics_snapshot_v1(self.app_data_dir(), input).await
    }

    pub async fn load_storage_usage_snapshot_json(&self) -> SonaCoreBindingResult<String> {
        storage_usage_bridge::load_storage_usage_snapshot_json(self.source()).await
    }

    pub async fn load_storage_usage_snapshot_v1(
        &self,
    ) -> SonaCoreBindingResult<FfiStorageUsageSnapshotV1> {
        storage_usage_bridge::load_storage_usage_snapshot_v1(self.source()).await
    }
}
