mod app_config_repository_bridge;
mod application_context;
#[cfg(test)]
mod application_context_tests;
mod asr_batch_bridge;
mod asr_bridge;
mod asr_streaming_bridge;
mod automation_bridge;
mod backup_bridge;
mod config_bridge;
mod dashboard_bridge;
mod diagnostics_bridge;
mod export_bridge;
mod facade;
mod history_mutation_bridge;
mod history_query_bridge;
mod json_bridge;
mod llm_bridge;
mod llm_runtime_bridge;
mod llm_task_bridge;
mod mapper;
mod model_bridge;
mod recovery_bridge;
mod runtime_bridge;
mod storage_usage_bridge;
mod sync_bridge;
mod sync_secret_store_bridge;
mod tag_bridge;
mod task_ledger_bridge;
pub use asr_batch_bridge::{
    FfiOnlineAsrApiKey, FfiOnlineAsrBatchProvider, FfiOnlineAsrBatchRequest,
    FfiOnlineAsrBatchResult,
};
pub use asr_streaming_bridge::{FfiAsrStreamingObserver, FfiAsrStreamingSession};
pub use facade::SonaCoreFacade;
pub use llm_task_bridge::FfiLlmTaskObserver;
pub use mapper::{
    FfiAsrEngine, FfiAsrInferenceMetric, FfiAsrMode, FfiAsrModelLoadMetric,
    FfiAsrStreamingErrorEvent, FfiAsrTranscriptUpdateEvent, FfiAutomationExportConfigV1,
    FfiAutomationProcessedInputV1, FfiAutomationProcessedRecordV1, FfiAutomationRepositoryInputV1,
    FfiAutomationRepositoryStateV1, FfiAutomationRuleInputV1, FfiAutomationRuleRecordV1,
    FfiAutomationRuleValidationResultV1, FfiAutomationStageConfigV1, FfiAutomationTagReferenceV1,
    FfiAutomationValidationExportConfigV1, FfiAutomationValidationRuleV1,
    FfiAutomationValidationStageConfigV1, FfiBatchSegmentationMode, FfiConfigMigrationResult,
    FfiHistoryAudioStatusV1, FfiHistoryCompleteLiveDraftRequestV1,
    FfiHistoryCreateLiveDraftRequestV1, FfiHistoryCreateTranscriptSnapshotRequestV1,
    FfiHistoryDeleteItemsRequestV1, FfiHistoryDraftSourcePatchV1, FfiHistoryDraftSourceV1,
    FfiHistoryItemKindV1, FfiHistoryItemMetaPatchV1, FfiHistoryItemRecordV1,
    FfiHistoryItemStatusV1, FfiHistoryReplaceTagAssignmentsRequestV1,
    FfiHistorySaveImportedFileRequestV1, FfiHistorySaveRecordingRequestV1,
    FfiHistorySearchMatchEntryV1, FfiHistoryTagCountEntryV1, FfiHistoryTrashItemsRequestV1,
    FfiHistoryUpdateItemMetaRequestV1, FfiHistoryUpdateTagAssignmentsRequestV1,
    FfiHistoryUpdateTranscriptRequestV1, FfiHistoryWorkspaceDateFilterV1,
    FfiHistoryWorkspaceFilterTypeV1, FfiHistoryWorkspaceItemCountsV1,
    FfiHistoryWorkspaceItemSearchMatchV1, FfiHistoryWorkspaceQueryRequestV1,
    FfiHistoryWorkspaceQueryResultV1, FfiHistoryWorkspaceScopeV1, FfiHistoryWorkspaceSearchRangeV1,
    FfiHistoryWorkspaceSearchSnippetV1, FfiHistoryWorkspaceSortOrderV1,
    FfiHistoryWorkspaceSummaryV1, FfiLiveRecordingDraftResultV1, FfiLlmCompletionResponse,
    FfiLlmConfig, FfiLlmExecutionMetadata, FfiLlmModality, FfiLlmModelMetadataSource,
    FfiLlmModelSummary, FfiLlmPromptChunk, FfiLlmProvider, FfiLlmProviderDefaults,
    FfiLlmProviderStrategy, FfiLlmResponseFormatKind, FfiLlmSegmentInput, FfiLlmTaskChunk,
    FfiLlmTaskFinal, FfiLlmTaskProgress, FfiLlmTaskText, FfiLlmTaskType, FfiLlmTokenUsage,
    FfiModelCatalogGroup, FfiModelCatalogModel, FfiModelCatalogPathMatchToken,
    FfiModelCatalogRestoreDefaults, FfiModelCatalogSection, FfiModelCatalogSectionType,
    FfiModelCatalogSelectedIds, FfiModelCatalogSelectionOptions, FfiModelCatalogSnapshot,
    FfiModelDependencyConfigKey, FfiModelDependencyRequest, FfiModelDependencyRequestsForModel,
    FfiModelIdByNormalizedPathEntry, FfiModelPathByIdEntry, FfiModelRules, FfiModelSelectionOption,
    FfiModelSelectionPaths, FfiOnlineAsrBatchCapability, FfiOnlineAsrCapability,
    FfiOnlineAsrLocalFileBatchMode, FfiOnlineAsrProvider, FfiOnlineAsrProviderRequest,
    FfiPolishSegmentsRequest, FfiPolishedSegment, FfiPresetModel, FfiRecoveredQueueItemV1,
    FfiRecoveredTranscriptSegmentV1, FfiRecoveredTranscriptTimingUnitV1,
    FfiRecoveredTranscriptTimingV1, FfiRecoveryFileStatV1, FfiRecoveryItemInputV1,
    FfiRecoveryItemStageV1, FfiRecoveryQueueStatusV1, FfiRecoveryResolutionV1,
    FfiRecoverySnapshotV1, FfiRecoverySourceV1, FfiRequiredCompanionModels,
    FfiResolvedModelDownload, FfiRuntimePathKind, FfiRuntimePathStatus, FfiSpeakerAttribution,
    FfiSpeakerCandidate, FfiSpeakerTag, FfiStringPatchV1, FfiSummarizeTranscriptRequest,
    FfiSummarySegmentInput, FfiSummaryTemplateConfig, FfiTagCreateInputV1, FfiTagDefaultsInputV1,
    FfiTagDefaultsPatchV1, FfiTagDefaultsV1, FfiTagRecordV1, FfiTagRepositorySnapshotV1,
    FfiTagUpdateInputV1, FfiTaskLedgerKindV1, FfiTaskLedgerPatchV1, FfiTaskLedgerRecordV1,
    FfiTaskLedgerSnapshotV1, FfiTaskLedgerStatusV1, FfiTimestampSupportHint, FfiTranscriptSegment,
    FfiTranscriptSnapshotMetadataV1, FfiTranscriptSnapshotReasonV1, FfiTranscriptSnapshotRecordV1,
    FfiTranscriptTiming, FfiTranscriptTimingLevel, FfiTranscriptTimingSource,
    FfiTranscriptTimingUnit, FfiTranscriptUpdate, FfiTranslateSegmentsRequest,
    FfiTranslatedSegment, FfiVolcengineDoubaoAsrConfig,
};
pub use sync_secret_store_bridge::FfiSyncSecretStore;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SonaCoreBindingError {
    #[error("{reason}")]
    InvalidInput { reason: String },
    #[error("{reason}")]
    Recovery { reason: String },
    #[error("{reason}")]
    TaskLedger { reason: String },
    #[error("{reason}")]
    Automation { reason: String },
    #[error("{reason}")]
    Tag { reason: String },
    #[error("{reason}")]
    AsrRuntime { code: String, reason: String },
    #[error("{reason}")]
    LlmRuntime {
        code: String,
        reason: String,
        retry_after_ms: Option<u64>,
    },
    #[error("{reason}")]
    ConfigRepository { reason: String },
    #[error("{reason}")]
    Dashboard { reason: String },
    #[error("{reason}")]
    Diagnostics { reason: String },
    #[error("{reason}")]
    StorageUsage { reason: String },
    #[error("{reason}")]
    Export { reason: String },
    #[error("{reason}")]
    HistoryQuery { reason: String },
    #[error("{reason}")]
    HistoryMutation { reason: String },
    #[error("{reason}")]
    Backup { reason: String },
    #[error("{reason}")]
    Sync { reason: String },
}

pub type SonaCoreBindingResult<T> = Result<T, SonaCoreBindingError>;

impl From<sona_core::ports::asr::SherpaError> for SonaCoreBindingError {
    fn from(error: sona_core::ports::asr::SherpaError) -> Self {
        let fallback_reason = error.to_string();
        let serialized = serde_json::to_value(&error).ok();
        let fields = serialized.as_ref().and_then(|value| {
            Some((
                value.get("code")?.as_str()?.to_string(),
                value.get("message")?.as_str()?.to_string(),
            ))
        });

        match fields {
            Some((code, reason)) => Self::AsrRuntime { code, reason },
            None => Self::AsrRuntime {
                code: "GENERIC_ERROR".to_string(),
                reason: fallback_reason,
            },
        }
    }
}

#[uniffi::export]
pub fn load_tag_repository_state_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_tag_repository_state_json(app_data_dir)
}

#[uniffi::export]
pub fn load_tag_repository_v1(
    app_data_dir: String,
) -> SonaCoreBindingResult<FfiTagRepositorySnapshotV1> {
    SonaCoreFacade::load_tag_repository_v1(app_data_dir)
}

#[uniffi::export]
pub fn replace_tags_json(app_data_dir: String, tags_json: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::replace_tags_json(app_data_dir, tags_json)
}

#[uniffi::export]
pub fn replace_tags_v1(
    app_data_dir: String,
    tags: Vec<FfiTagRecordV1>,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::replace_tags_v1(app_data_dir, tags)
}

#[uniffi::export]
pub fn create_tag_json(app_data_dir: String, input_json: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::create_tag_json(app_data_dir, input_json)
}

#[uniffi::export]
pub fn create_tag_v1(
    app_data_dir: String,
    input: FfiTagCreateInputV1,
) -> SonaCoreBindingResult<FfiTagRecordV1> {
    SonaCoreFacade::create_tag_v1(app_data_dir, input)
}

#[uniffi::export]
pub fn update_tag_json(
    app_data_dir: String,
    tag_id: String,
    updates_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::update_tag_json(app_data_dir, tag_id, updates_json)
}

#[uniffi::export]
pub fn update_tag_v1(
    app_data_dir: String,
    tag_id: String,
    updates: FfiTagUpdateInputV1,
) -> SonaCoreBindingResult<Option<FfiTagRecordV1>> {
    SonaCoreFacade::update_tag_v1(app_data_dir, tag_id, updates)
}

#[uniffi::export]
pub fn delete_tag(app_data_dir: String, tag_id: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::delete_tag(app_data_dir, tag_id)
}

#[uniffi::export]
pub fn delete_tag_v1(app_data_dir: String, tag_id: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::delete_tag_v1(app_data_dir, tag_id)
}

#[uniffi::export]
pub fn reorder_tags_json(
    app_data_dir: String,
    tag_ids_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::reorder_tags_json(app_data_dir, tag_ids_json)
}

#[uniffi::export]
pub fn reorder_tags_v1(
    app_data_dir: String,
    tag_ids: Vec<String>,
) -> SonaCoreBindingResult<Vec<FfiTagRecordV1>> {
    SonaCoreFacade::reorder_tags_v1(app_data_dir, tag_ids)
}

#[uniffi::export]
pub fn set_active_tag_id(
    app_data_dir: String,
    tag_id: Option<String>,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::set_active_tag_id(app_data_dir, tag_id)
}

#[uniffi::export]
pub fn set_active_tag_id_v1(
    app_data_dir: String,
    tag_id: Option<String>,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::set_active_tag_id_v1(app_data_dir, tag_id)
}

#[uniffi::export]
pub fn load_recovery_snapshot_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_recovery_snapshot_json(app_data_dir)
}

#[uniffi::export]
pub fn load_recovery_snapshot_v1(
    app_data_dir: String,
) -> SonaCoreBindingResult<FfiRecoverySnapshotV1> {
    SonaCoreFacade::load_recovery_snapshot_v1(app_data_dir)
}

#[uniffi::export]
pub fn save_recovery_snapshot_json(
    app_data_dir: String,
    items_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::save_recovery_snapshot_json(app_data_dir, items_json)
}

#[uniffi::export]
pub fn save_recovery_snapshot_v1(
    app_data_dir: String,
    items: Vec<FfiRecoveryItemInputV1>,
) -> SonaCoreBindingResult<FfiRecoverySnapshotV1> {
    SonaCoreFacade::save_recovery_snapshot_v1(app_data_dir, items)
}

#[uniffi::export]
pub fn persist_recovery_queue_snapshot_json(
    app_data_dir: String,
    queue_items_json: String,
    resolved_ids: Vec<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::persist_recovery_queue_snapshot_json(
        app_data_dir,
        queue_items_json,
        resolved_ids,
    )
}

#[uniffi::export]
pub fn persist_recovery_queue_snapshot_v1(
    app_data_dir: String,
    queue_items: Vec<FfiRecoveryItemInputV1>,
    resolved_ids: Vec<String>,
) -> SonaCoreBindingResult<FfiRecoverySnapshotV1> {
    SonaCoreFacade::persist_recovery_queue_snapshot_v1(app_data_dir, queue_items, resolved_ids)
}

#[uniffi::export]
pub fn load_task_ledger_snapshot_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_task_ledger_snapshot_json(app_data_dir)
}

#[uniffi::export]
pub fn load_task_ledger_snapshot_v1(
    app_data_dir: String,
) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
    SonaCoreFacade::load_task_ledger_snapshot_v1(app_data_dir)
}

#[uniffi::export]
pub fn upsert_task_ledger_record_json(
    app_data_dir: String,
    record_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::upsert_task_ledger_record_json(app_data_dir, record_json)
}

#[uniffi::export]
pub fn upsert_task_ledger_record_v1(
    app_data_dir: String,
    record: FfiTaskLedgerRecordV1,
) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
    SonaCoreFacade::upsert_task_ledger_record_v1(app_data_dir, record)
}

#[uniffi::export]
pub fn patch_task_ledger_record_json(
    app_data_dir: String,
    id: String,
    patch_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::patch_task_ledger_record_json(app_data_dir, id, patch_json)
}

#[uniffi::export]
pub fn patch_task_ledger_record_v1(
    app_data_dir: String,
    id: String,
    patch: FfiTaskLedgerPatchV1,
) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
    SonaCoreFacade::patch_task_ledger_record_v1(app_data_dir, id, patch)
}

#[uniffi::export]
pub fn remove_task_ledger_record_json(
    app_data_dir: String,
    id: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::remove_task_ledger_record_json(app_data_dir, id)
}

#[uniffi::export]
pub fn remove_task_ledger_record_v1(
    app_data_dir: String,
    id: String,
) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
    SonaCoreFacade::remove_task_ledger_record_v1(app_data_dir, id)
}

#[uniffi::export]
pub fn clear_resolved_task_ledger_records_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::clear_resolved_task_ledger_records_json(app_data_dir)
}

#[uniffi::export]
pub fn clear_resolved_task_ledger_records_v1(
    app_data_dir: String,
) -> SonaCoreBindingResult<FfiTaskLedgerSnapshotV1> {
    SonaCoreFacade::clear_resolved_task_ledger_records_v1(app_data_dir)
}

#[uniffi::export]
pub fn load_automation_repository_state_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_automation_repository_state_json(app_data_dir)
}

#[uniffi::export]
pub fn load_automation_repository_state_v1(
    app_data_dir: String,
) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
    SonaCoreFacade::load_automation_repository_state_v1(app_data_dir)
}

#[uniffi::export]
pub fn replace_automation_rules_json(
    app_data_dir: String,
    rules_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::replace_automation_rules_json(app_data_dir, rules_json)
}

#[uniffi::export]
pub fn replace_automation_rules_v1(
    app_data_dir: String,
    rules: Vec<FfiAutomationRuleInputV1>,
) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
    SonaCoreFacade::replace_automation_rules_v1(app_data_dir, rules)
}

#[uniffi::export]
pub fn replace_automation_processed_entries_json(
    app_data_dir: String,
    entries_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::replace_automation_processed_entries_json(app_data_dir, entries_json)
}

#[uniffi::export]
pub fn replace_automation_processed_entries_v1(
    app_data_dir: String,
    entries: Vec<FfiAutomationProcessedInputV1>,
) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
    SonaCoreFacade::replace_automation_processed_entries_v1(app_data_dir, entries)
}

#[uniffi::export]
pub fn replace_automation_repository_state_json(
    app_data_dir: String,
    state_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::replace_automation_repository_state_json(app_data_dir, state_json)
}

#[uniffi::export]
pub fn replace_automation_repository_state_v1(
    app_data_dir: String,
    input: FfiAutomationRepositoryInputV1,
) -> SonaCoreBindingResult<FfiAutomationRepositoryStateV1> {
    SonaCoreFacade::replace_automation_repository_state_v1(app_data_dir, input)
}

#[uniffi::export]
pub fn validate_automation_rule_activation_json(
    rule_json: String,
    global_config_json: String,
    project_json: Option<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::validate_automation_rule_activation_json(
        rule_json,
        global_config_json,
        project_json,
    )
}

#[uniffi::export]
pub fn validate_automation_rule_activation_v1(
    rule: FfiAutomationValidationRuleV1,
    global_config_json: String,
    tags: Vec<FfiAutomationTagReferenceV1>,
) -> SonaCoreBindingResult<FfiAutomationRuleValidationResultV1> {
    SonaCoreFacade::validate_automation_rule_activation_v1(rule, global_config_json, tags)
}

#[uniffi::export]
pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::normalize_export_format(value)
}

#[uniffi::export]
pub async fn export_transcript_file_json(input_json: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::export_transcript_file_json(input_json).await
}

#[uniffi::export]
pub async fn export_backup_archive_json(
    app_data_dir: String,
    archive_path: String,
    app_version: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::export_backup_archive_json(app_data_dir, archive_path, app_version).await
}

#[uniffi::export]
pub async fn inspect_backup_archive_json(archive_path: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::inspect_backup_archive_json(archive_path).await
}

#[uniffi::export]
pub async fn import_backup_archive_json(
    app_data_dir: String,
    archive_path: String,
    default_rule_set_name: String,
    confirm_replace: bool,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::import_backup_archive_json(
        app_data_dir,
        archive_path,
        default_rule_set_name,
        confirm_replace,
    )
    .await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_test_provider_json(config_json: String) -> SonaCoreBindingResult<String> {
    sync_bridge::test_provider_json(config_json).await
}

#[uniffi::export]
pub fn register_sync_secret_store(store: std::sync::Arc<dyn FfiSyncSecretStore>) {
    sync_bridge::register_sync_secret_store(store);
}

#[uniffi::export]
pub fn register_sync_secret_store_for_app_data_dir(
    app_data_dir: String,
    store: std::sync::Arc<dyn FfiSyncSecretStore>,
) -> SonaCoreBindingResult<()> {
    sync_bridge::register_sync_secret_store_for_app_data_dir(&app_data_dir, store)
}

#[uniffi::export]
pub fn release_application_context(app_data_dir: String) -> SonaCoreBindingResult<bool> {
    application_context::release_application_context(app_data_dir).map_err(|error| {
        SonaCoreBindingError::InvalidInput {
            reason: error.to_string(),
        }
    })
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_get_status_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    sync_bridge::get_status_json(app_data_dir).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_create_vault_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    sync_bridge::create_vault_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_preview_join_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    sync_bridge::preview_join_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_join_vault_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    sync_bridge::join_vault_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_unlock_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    sync_bridge::unlock_json(app_data_dir, request_json, false).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_unlock_with_recovery_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    sync_bridge::unlock_json(app_data_dir, request_json, true).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_lock(app_data_dir: String) -> SonaCoreBindingResult<()> {
    sync_bridge::lock(app_data_dir).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_set_paused_json(
    app_data_dir: String,
    paused: bool,
) -> SonaCoreBindingResult<String> {
    sync_bridge::set_paused_json(app_data_dir, paused).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_disconnect_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    sync_bridge::disconnect_json(app_data_dir).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_run_now_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    sync_bridge::run_now_json(app_data_dir).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_change_preset_json(
    app_data_dir: String,
    preset_json: String,
    confirm_shrink: bool,
) -> SonaCoreBindingResult<String> {
    sync_bridge::change_preset_json(app_data_dir, preset_json, confirm_shrink).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_change_master_password_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<()> {
    sync_bridge::change_master_password_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn sync_generate_recovery_key(app_data_dir: String) -> SonaCoreBindingResult<String> {
    sync_bridge::generate_recovery_key(app_data_dir).await
}

#[uniffi::export]
pub fn sync_list_conflicts_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    sync_bridge::list_conflicts_json(app_data_dir)
}

#[uniffi::export]
pub fn sync_get_conflict_json(
    app_data_dir: String,
    conflict_id: String,
) -> SonaCoreBindingResult<String> {
    sync_bridge::get_conflict_json(app_data_dir, conflict_id)
}

#[uniffi::export]
pub fn sync_resolve_conflict_json(
    app_data_dir: String,
    conflict_id: String,
    resolution_json: String,
) -> SonaCoreBindingResult<()> {
    sync_bridge::resolve_conflict_json(app_data_dir, conflict_id, resolution_json)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn list_history_items_json(
    app_data_dir: String,
    limit: Option<u64>,
    offset: Option<u64>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::list_history_items_json(app_data_dir, limit, offset).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn list_history_items_v1(
    app_data_dir: String,
    limit: Option<u64>,
    offset: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiHistoryItemRecordV1>> {
    SonaCoreFacade::list_history_items_v1(app_data_dir, limit, offset).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn query_history_workspace_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::query_history_workspace_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn query_history_workspace_v1(
    app_data_dir: String,
    request: FfiHistoryWorkspaceQueryRequestV1,
) -> SonaCoreBindingResult<FfiHistoryWorkspaceQueryResultV1> {
    SonaCoreFacade::query_history_workspace_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn load_history_transcript_json(
    app_data_dir: String,
    history_id: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_history_transcript_json(app_data_dir, history_id).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn load_history_transcript_v1(
    app_data_dir: String,
    history_id: String,
) -> SonaCoreBindingResult<Option<Vec<FfiTranscriptSegment>>> {
    SonaCoreFacade::load_history_transcript_v1(app_data_dir, history_id).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn list_history_transcript_snapshots_json(
    app_data_dir: String,
    history_id: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::list_history_transcript_snapshots_json(app_data_dir, history_id).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn list_history_transcript_snapshots_v1(
    app_data_dir: String,
    history_id: String,
) -> SonaCoreBindingResult<Vec<FfiTranscriptSnapshotMetadataV1>> {
    SonaCoreFacade::list_history_transcript_snapshots_v1(app_data_dir, history_id).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn load_history_transcript_snapshot_json(
    app_data_dir: String,
    history_id: String,
    snapshot_id: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_history_transcript_snapshot_json(app_data_dir, history_id, snapshot_id)
        .await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn load_history_transcript_snapshot_v1(
    app_data_dir: String,
    history_id: String,
    snapshot_id: String,
) -> SonaCoreBindingResult<Option<FfiTranscriptSnapshotRecordV1>> {
    SonaCoreFacade::load_history_transcript_snapshot_v1(app_data_dir, history_id, snapshot_id).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn create_history_live_draft_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::create_history_live_draft_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn create_history_live_draft_v1(
    app_data_dir: String,
    request: FfiHistoryCreateLiveDraftRequestV1,
) -> SonaCoreBindingResult<FfiLiveRecordingDraftResultV1> {
    SonaCoreFacade::create_history_live_draft_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn complete_history_live_draft_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::complete_history_live_draft_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn complete_history_live_draft_v1(
    app_data_dir: String,
    request: FfiHistoryCompleteLiveDraftRequestV1,
) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
    SonaCoreFacade::complete_history_live_draft_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn save_history_recording_json(
    app_data_dir: String,
    request_json: String,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::save_history_recording_json(
        app_data_dir,
        request_json,
        audio_bytes,
        native_audio_path,
    )
    .await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn save_history_recording_v1(
    app_data_dir: String,
    request: FfiHistorySaveRecordingRequestV1,
) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
    SonaCoreFacade::save_history_recording_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn save_history_imported_file_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::save_history_imported_file_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn save_history_imported_file_v1(
    app_data_dir: String,
    request: FfiHistorySaveImportedFileRequestV1,
) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
    SonaCoreFacade::save_history_imported_file_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn delete_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::delete_history_items_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn trash_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::trash_history_items_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn trash_history_items_v1(
    app_data_dir: String,
    request: FfiHistoryTrashItemsRequestV1,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::trash_history_items_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn restore_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::restore_history_items_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn restore_history_items_v1(
    app_data_dir: String,
    request: FfiHistoryDeleteItemsRequestV1,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::restore_history_items_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn purge_history_items_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::purge_history_items_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn purge_history_items_v1(
    app_data_dir: String,
    request: FfiHistoryDeleteItemsRequestV1,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::purge_history_items_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_transcript_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::update_history_transcript_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_transcript_v1(
    app_data_dir: String,
    request: FfiHistoryUpdateTranscriptRequestV1,
) -> SonaCoreBindingResult<FfiHistoryItemRecordV1> {
    SonaCoreFacade::update_history_transcript_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn create_history_transcript_snapshot_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::create_history_transcript_snapshot_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn create_history_transcript_snapshot_v1(
    app_data_dir: String,
    request: FfiHistoryCreateTranscriptSnapshotRequestV1,
) -> SonaCoreBindingResult<FfiTranscriptSnapshotMetadataV1> {
    SonaCoreFacade::create_history_transcript_snapshot_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_item_meta_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::update_history_item_meta_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_item_meta_v1(
    app_data_dir: String,
    request: FfiHistoryUpdateItemMetaRequestV1,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::update_history_item_meta_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_project_assignments_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::update_history_project_assignments_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_tag_assignments_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::update_history_tag_assignments_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn update_history_tag_assignments_v1(
    app_data_dir: String,
    request: FfiHistoryUpdateTagAssignmentsRequestV1,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::update_history_tag_assignments_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn replace_history_tag_assignments_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::replace_history_tag_assignments_json(app_data_dir, request_json).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn replace_history_tag_assignments_v1(
    app_data_dir: String,
    request: FfiHistoryReplaceTagAssignmentsRequestV1,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::replace_history_tag_assignments_v1(app_data_dir, request).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn reassign_history_project_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::reassign_history_project_json(app_data_dir, request_json).await
}

#[uniffi::export]
pub fn default_vad_model_id() -> String {
    SonaCoreFacade::default_vad_model_id()
}

#[uniffi::export]
pub fn default_punctuation_model_id() -> String {
    SonaCoreFacade::default_punctuation_model_id()
}

#[uniffi::export]
pub fn preset_model_name(model_id: String) -> Option<String> {
    SonaCoreFacade::preset_model_name(model_id)
}

#[uniffi::export]
pub fn preset_models() -> Vec<FfiPresetModel> {
    SonaCoreFacade::preset_models()
}

#[uniffi::export]
pub fn model_catalog_snapshot(
    models_dir: String,
    installed_model_ids: Vec<String>,
) -> FfiModelCatalogSnapshot {
    SonaCoreFacade::model_catalog_snapshot(models_dir, installed_model_ids)
}

#[uniffi::export]
pub fn model_catalog_selected_ids(
    models_dir: String,
    installed_model_ids: Vec<String>,
    paths: FfiModelSelectionPaths,
) -> FfiModelCatalogSelectedIds {
    SonaCoreFacade::model_catalog_selected_ids(models_dir, installed_model_ids, paths)
}

#[uniffi::export]
pub fn resolve_model_download(
    model_id: String,
    models_dir: String,
) -> SonaCoreBindingResult<FfiResolvedModelDownload> {
    SonaCoreFacade::resolve_model_download(model_id, models_dir)
}

#[uniffi::export]
pub fn resolve_gpu_acceleration(value: Option<String>) -> SonaCoreBindingResult<Option<String>> {
    SonaCoreFacade::resolve_gpu_acceleration(value)
}

#[uniffi::export]
pub fn default_config_json() -> String {
    SonaCoreFacade::default_config_json()
}

#[uniffi::export]
pub fn migrate_app_config_json(
    saved_config_json: Option<String>,
    legacy_config_json: Option<String>,
    default_rule_set_name: String,
) -> SonaCoreBindingResult<FfiConfigMigrationResult> {
    SonaCoreFacade::migrate_app_config_json(
        saved_config_json,
        legacy_config_json,
        default_rule_set_name,
    )
}

#[uniffi::export]
pub fn resolve_effective_config_json(
    global_config_json: String,
    project_json: Option<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::resolve_effective_config_json(global_config_json, project_json)
}

#[uniffi::export]
pub fn load_app_config_json(app_data_dir: String) -> SonaCoreBindingResult<Option<String>> {
    SonaCoreFacade::load_app_config_json(app_data_dir)
}

#[uniffi::export]
pub fn save_app_config_json(
    app_data_dir: String,
    config_json: String,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::save_app_config_json(app_data_dir, config_json)
}

#[uniffi::export]
pub fn get_app_setting_json(
    app_data_dir: String,
    key: String,
) -> SonaCoreBindingResult<Option<String>> {
    SonaCoreFacade::get_app_setting_json(app_data_dir, key)
}

#[uniffi::export]
pub fn set_app_setting_json(
    app_data_dir: String,
    key: String,
    value_json: String,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::set_app_setting_json(app_data_dir, key, value_json)
}

#[uniffi::export]
pub async fn load_dashboard_snapshot_json(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_dashboard_snapshot_json(app_data_dir, deep).await
}

#[uniffi::export]
pub async fn load_diagnostics_snapshot_json(
    app_data_dir: String,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_diagnostics_snapshot_json(app_data_dir, input_json).await
}

#[uniffi::export]
pub async fn load_storage_usage_snapshot_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::load_storage_usage_snapshot_json(app_data_dir).await
}

#[uniffi::export]
pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
    SonaCoreFacade::runtime_path_status(path)
}

#[uniffi::export]
pub fn create_online_asr_streaming_session(
    instance_id: String,
    request_json: String,
    observer: std::sync::Arc<dyn FfiAsrStreamingObserver>,
) -> SonaCoreBindingResult<std::sync::Arc<FfiAsrStreamingSession>> {
    SonaCoreFacade::create_online_asr_streaming_session(instance_id, request_json, observer)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn create_asr_streaming_session(
    instance_id: String,
    request_json: String,
    observer: std::sync::Arc<dyn FfiAsrStreamingObserver>,
) -> SonaCoreBindingResult<std::sync::Arc<FfiAsrStreamingSession>> {
    SonaCoreFacade::create_asr_streaming_session(instance_id, request_json, observer).await
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn transcribe_online_asr_batch(
    request: FfiOnlineAsrBatchRequest,
) -> SonaCoreBindingResult<FfiOnlineAsrBatchResult> {
    SonaCoreFacade::transcribe_online_asr_batch(request).await
}

#[uniffi::export]
pub fn default_batch_segmentation_mode() -> FfiBatchSegmentationMode {
    SonaCoreFacade::default_batch_segmentation_mode()
}

#[uniffi::export]
pub fn online_asr_providers() -> Vec<FfiOnlineAsrProvider> {
    SonaCoreFacade::online_asr_providers()
}

#[uniffi::export]
pub fn find_online_asr_provider(provider_id: String) -> Option<FfiOnlineAsrProvider> {
    SonaCoreFacade::find_online_asr_provider(provider_id)
}

#[uniffi::export]
pub fn online_asr_provider_request(
    provider_id: String,
    profile_id: String,
    config_json: String,
) -> SonaCoreBindingResult<FfiOnlineAsrProviderRequest> {
    SonaCoreFacade::online_asr_provider_request(provider_id, profile_id, config_json)
}

#[uniffi::export]
pub fn volcengine_doubao_asr_config_from_json(
    config_json: String,
) -> SonaCoreBindingResult<FfiVolcengineDoubaoAsrConfig> {
    SonaCoreFacade::volcengine_doubao_asr_config_from_json(config_json)
}

#[uniffi::export]
pub fn llm_providers() -> Vec<FfiLlmProvider> {
    SonaCoreFacade::llm_providers()
}

#[uniffi::export]
pub fn find_llm_provider_by_id_or_alias(id_or_alias: String) -> Option<FfiLlmProvider> {
    SonaCoreFacade::find_llm_provider_by_id_or_alias(id_or_alias)
}

#[uniffi::export]
pub fn llm_config_from_json(config_json: String) -> SonaCoreBindingResult<FfiLlmConfig> {
    SonaCoreFacade::llm_config_from_json(config_json)
}

#[uniffi::export]
pub async fn complete_llm_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiLlmCompletionResponse> {
    SonaCoreFacade::complete_llm_json(request_json).await
}

#[uniffi::export]
pub async fn list_llm_models_json(
    request_json: String,
) -> SonaCoreBindingResult<Vec<FfiLlmModelSummary>> {
    SonaCoreFacade::list_llm_models_json(request_json).await
}

#[uniffi::export]
pub async fn describe_llm_model_json(
    config_json: String,
) -> SonaCoreBindingResult<Option<FfiLlmModelSummary>> {
    SonaCoreFacade::describe_llm_model_json(config_json).await
}

#[uniffi::export]
pub async fn run_llm_polish_json(
    request_json: String,
    observer: std::sync::Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    SonaCoreFacade::run_llm_polish_json(request_json, observer).await
}

#[uniffi::export]
pub async fn run_llm_translate_json(
    request_json: String,
    observer: std::sync::Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    SonaCoreFacade::run_llm_translate_json(request_json, observer).await
}

#[uniffi::export]
pub async fn run_llm_summary_json(
    request_json: String,
    observer: std::sync::Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    SonaCoreFacade::run_llm_summary_json(request_json, observer).await
}

#[uniffi::export]
pub fn validate_llm_config_json(config_json: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::validate_llm_config_json(config_json)
}

#[uniffi::export]
pub fn validate_llm_generate_request_json(request_json: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::validate_llm_generate_request_json(request_json)
}

#[uniffi::export]
pub fn validate_polish_segments_request_json(request_json: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::validate_polish_segments_request_json(request_json)
}

#[uniffi::export]
pub fn validate_translate_segments_request_json(request_json: String) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::validate_translate_segments_request_json(request_json)
}

#[uniffi::export]
pub fn validate_summarize_transcript_request_json(
    request_json: String,
) -> SonaCoreBindingResult<()> {
    SonaCoreFacade::validate_summarize_transcript_request_json(request_json)
}

#[uniffi::export]
pub fn llm_segment_inputs_from_transcript_json(
    segments_json: String,
) -> SonaCoreBindingResult<Vec<FfiLlmSegmentInput>> {
    SonaCoreFacade::llm_segment_inputs_from_transcript_json(segments_json)
}

#[uniffi::export]
pub fn summary_segment_inputs_from_transcript_json(
    segments_json: String,
) -> SonaCoreBindingResult<Vec<FfiSummarySegmentInput>> {
    SonaCoreFacade::summary_segment_inputs_from_transcript_json(segments_json)
}

#[uniffi::export]
pub fn merge_translated_items_into_transcript_json(
    segments_json: String,
    items_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::merge_translated_items_into_transcript_json(segments_json, items_json)
}

#[uniffi::export]
pub fn merge_polished_items_into_transcript_json(
    segments_json: String,
    items_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::merge_polished_items_into_transcript_json(segments_json, items_json)
}

#[uniffi::export]
pub fn summary_source_fingerprint_from_transcript_json(
    segments_json: String,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::summary_source_fingerprint_from_transcript_json(segments_json)
}

#[uniffi::export]
pub fn build_polish_prompt_json(
    segments_json: String,
    context: Option<String>,
    keywords: Option<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::build_polish_prompt_json(segments_json, context, keywords)
}

#[uniffi::export]
pub fn build_translate_prompt_json(
    segments_json: String,
    target_language: String,
    target_language_name: Option<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::build_translate_prompt_json(
        segments_json,
        target_language,
        target_language_name,
    )
}

#[uniffi::export]
pub fn build_summary_chunk_prompt_json(
    template_json: String,
    segments_json: String,
    chunk_number: u64,
    total_chunks: u64,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::build_summary_chunk_prompt_json(
        template_json,
        segments_json,
        chunk_number,
        total_chunks,
    )
}

#[uniffi::export]
pub fn build_summary_finalize_prompt_json(
    template_json: String,
    partial_summaries: Vec<String>,
) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::build_summary_finalize_prompt_json(template_json, partial_summaries)
}

#[uniffi::export]
pub fn plan_polish_prompt_chunks_json(
    segments_json: String,
    context: Option<String>,
    keywords: Option<String>,
    chunk_size: Option<u64>,
    prompt_char_budget: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
    SonaCoreFacade::plan_polish_prompt_chunks_json(
        segments_json,
        context,
        keywords,
        chunk_size,
        prompt_char_budget,
    )
}

#[uniffi::export]
pub fn plan_translate_prompt_chunks_json(
    segments_json: String,
    target_language: String,
    target_language_name: Option<String>,
    chunk_size: Option<u64>,
    prompt_char_budget: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
    SonaCoreFacade::plan_translate_prompt_chunks_json(
        segments_json,
        target_language,
        target_language_name,
        chunk_size,
        prompt_char_budget,
    )
}

#[uniffi::export]
pub fn plan_summary_prompt_chunks_json(
    template_json: String,
    segments_json: String,
    chunk_char_budget: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
    SonaCoreFacade::plan_summary_prompt_chunks_json(template_json, segments_json, chunk_char_budget)
}

#[uniffi::export]
pub fn parse_polish_chunk_json(
    response_text: String,
    expected_segments_json: String,
    chunk_number: u64,
) -> SonaCoreBindingResult<Vec<FfiPolishedSegment>> {
    SonaCoreFacade::parse_polish_chunk_json(response_text, expected_segments_json, chunk_number)
}

#[uniffi::export]
pub fn parse_translate_chunk_json(
    response_text: String,
    expected_segments_json: String,
    chunk_number: u64,
) -> SonaCoreBindingResult<Vec<FfiTranslatedSegment>> {
    SonaCoreFacade::parse_translate_chunk_json(response_text, expected_segments_json, chunk_number)
}

#[uniffi::export]
pub fn polish_segments_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiPolishSegmentsRequest> {
    SonaCoreFacade::polish_segments_request_from_json(request_json)
}

#[uniffi::export]
pub fn translate_segments_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiTranslateSegmentsRequest> {
    SonaCoreFacade::translate_segments_request_from_json(request_json)
}

#[uniffi::export]
pub fn summarize_transcript_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiSummarizeTranscriptRequest> {
    SonaCoreFacade::summarize_transcript_request_from_json(request_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::models::preset_models::{
        DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model,
    };
    use sona_core::ports::asr::VOLCENGINE_DOUBAO_PROVIDER_ID;

    #[test]
    fn facade_returns_owned_binding_safe_values_from_core() {
        assert_eq!(
            SonaCoreFacade::normalize_export_format("SRT".to_string()).unwrap(),
            "srt"
        );
        assert_eq!(SonaCoreFacade::default_vad_model_id(), "silero-vad");
        assert_eq!(
            SonaCoreFacade::preset_model_name("silero-vad".to_string()).as_deref(),
            find_preset_model("silero-vad").map(|model| model.name.as_str())
        );
    }

    #[test]
    fn facade_maps_core_errors_to_binding_errors() {
        let error = SonaCoreFacade::normalize_export_format("docx".to_string()).unwrap_err();
        assert_eq!(error.to_string(), "Unsupported export format: docx");
    }

    #[test]
    fn facade_resolves_gpu_acceleration_config_values() {
        assert_eq!(
            SonaCoreFacade::resolve_gpu_acceleration(None)
                .unwrap()
                .as_deref(),
            Some("auto")
        );
        assert_eq!(
            SonaCoreFacade::resolve_gpu_acceleration(Some(" CUDA ".to_string()))
                .unwrap()
                .as_deref(),
            Some("cuda")
        );

        let error =
            SonaCoreFacade::resolve_gpu_acceleration(Some("metal".to_string())).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("gpu_acceleration must be one of")
        );
    }

    #[test]
    fn facade_exposes_llm_provider_manifest_for_mobile() {
        let providers = SonaCoreFacade::llm_providers();
        let open_ai = providers
            .iter()
            .find(|provider| provider.id == "open_ai")
            .expect("OpenAI provider should be exported");

        assert_eq!(open_ai.aliases, vec!["openai"]);
        assert_eq!(open_ai.defaults.api_host, "https://api.openai.com");
        assert_eq!(open_ai.defaults.api_path, None);
        assert_eq!(open_ai.defaults.api_version, None);

        let by_alias = SonaCoreFacade::find_llm_provider_by_id_or_alias("openai".to_string())
            .expect("OpenAI provider should be found by alias");
        assert_eq!(by_alias.id, open_ai.id);

        let azure = SonaCoreFacade::find_llm_provider_by_id_or_alias("azure_openai".to_string())
            .expect("Azure OpenAI provider should be found by id");
        assert_eq!(azure.defaults.api_host, "");
        assert_eq!(azure.defaults.api_version.as_deref(), Some("2024-10-21"));
        assert!(
            SonaCoreFacade::find_llm_provider_by_id_or_alias("missing-provider".to_string())
                .is_none()
        );
    }

    #[test]
    fn top_level_exports_delegate_to_core_facade() {
        assert_eq!(normalize_export_format("VTT".to_string()).unwrap(), "vtt");
        assert_eq!(default_punctuation_model_id(), DEFAULT_PUNCTUATION_MODEL_ID);
        assert_eq!(
            preset_model_name("missing-model".to_string()).as_deref(),
            None
        );
    }

    #[test]
    fn runtime_path_status_returns_binding_safe_record() {
        let missing_path = std::env::temp_dir()
            .join("sona-uniffi-bind-missing-runtime-path")
            .to_string_lossy()
            .into_owned();

        let status = SonaCoreFacade::runtime_path_status(missing_path.clone());

        assert_eq!(status.path, missing_path);
        assert_eq!(status.kind, FfiRuntimePathKind::Missing);
        assert_eq!(status.error, None);
    }

    #[test]
    fn facade_maps_asr_contract_values_to_binding_safe_records() {
        assert_eq!(
            SonaCoreFacade::default_batch_segmentation_mode(),
            FfiBatchSegmentationMode::Vad
        );

        let provider = SonaCoreFacade::online_asr_provider_request(
            "volcengine".to_string(),
            "default".to_string(),
            r#"{"apiKey":"secret"}"#.to_string(),
        )
        .unwrap();

        assert_eq!(provider.provider_id, "volcengine");
        assert_eq!(provider.profile_id, "default");
        assert_eq!(provider.config_json, r#"{"apiKey":"secret"}"#);
    }

    #[test]
    fn facade_exposes_online_asr_provider_manifest_for_mobile() {
        let providers = SonaCoreFacade::online_asr_providers();
        let volcengine = providers
            .iter()
            .find(|provider| provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID)
            .expect("Volcengine Doubao provider should be exported");

        assert_eq!(volcengine.profile_id, "volcengine-doubao-default");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&volcengine.defaults_json).unwrap()["apiKey"],
            serde_json::json!("")
        );
        assert_eq!(volcengine.streaming.supported, None);
        assert!(volcengine.streaming.requires_api_key);
        assert_eq!(
            volcengine.streaming.required_config_fields,
            vec!["streamingEndpoint", "streamingResourceId"]
        );
        assert!(volcengine.batch.requires_api_key);
        assert!(volcengine.batch.local_file_mode.supported);
        assert_eq!(
            volcengine.batch.local_file_mode.endpoint,
            "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash"
        );
        assert_eq!(
            volcengine.batch.local_file_mode.resource_id,
            "volc.bigasr.auc_turbo"
        );

        let found = SonaCoreFacade::find_online_asr_provider(VOLCENGINE_DOUBAO_PROVIDER_ID.into())
            .expect("Volcengine Doubao provider should be found by id");
        assert_eq!(found.id, volcengine.id);
        assert_eq!(found.defaults_json, volcengine.defaults_json);
        assert!(SonaCoreFacade::find_online_asr_provider("missing".to_string()).is_none());
    }

    #[test]
    fn facade_maps_volcengine_asr_config_from_json() {
        let config = SonaCoreFacade::volcengine_doubao_asr_config_from_json(
            r#"{
                "apiKey": "secret",
                "streamingEndpoint": "wss://stream",
                "streamingResourceId": "stream-resource",
                "batchEndpoint": "https://batch",
                "batchResourceId": "batch-resource"
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(config.api_key, "secret");
        assert_eq!(config.streaming_endpoint, "wss://stream");
        assert_eq!(config.streaming_resource_id, "stream-resource");
        assert_eq!(config.batch_endpoint, "https://batch");
        assert_eq!(config.batch_resource_id, "batch-resource");
    }

    #[test]
    fn facade_rejects_invalid_asr_provider_config_json() {
        let error = SonaCoreFacade::online_asr_provider_request(
            "volcengine".to_string(),
            "default".to_string(),
            "{bad-json".to_string(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Invalid ASR provider config JSON")
        );
    }

    #[test]
    fn facade_maps_llm_config_and_segment_task_requests_from_json() {
        let config = SonaCoreFacade::llm_config_from_json(
            r#"{
                "provider": "open_ai",
                "baseUrl": "https://api.openai.com",
                "apiKey": "secret",
                "model": "gpt-4o-mini",
                "temperature": 0.2,
                "timeoutSeconds": 30
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(config.provider_id, "open_ai");
        assert_eq!(config.strategy, FfiLlmProviderStrategy::OpenAi);
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.temperature, Some(0.2));
        assert_eq!(config.timeout_seconds, Some(30));

        let polish = SonaCoreFacade::polish_segments_request_from_json(
            r#"{
                "taskId": "polish-1",
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.openai.com",
                    "apiKey": "secret",
                    "model": "gpt-4o-mini"
                },
                "segments": [{"id": "s1", "text": "hello"}],
                "chunkSize": 4,
                "context": "meeting",
                "keywords": "roadmap"
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(polish.task_id, "polish-1");
        assert_eq!(polish.segments[0].id, "s1");
        assert_eq!(polish.chunk_size, Some(4));
        assert_eq!(polish.context.as_deref(), Some("meeting"));

        let translate = SonaCoreFacade::translate_segments_request_from_json(
            r#"{
                "taskId": "translate-1",
                "config": {
                    "provider": "google_translate_free",
                    "baseUrl": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": "",
                    "model": "translate"
                },
                "segments": [{"id": "s1", "text": "hello"}],
                "targetLanguage": "ja",
                "targetLanguageName": "Japanese"
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(
            translate.config.strategy,
            FfiLlmProviderStrategy::GoogleTranslateFree
        );
        assert_eq!(translate.target_language, "ja");
        assert_eq!(translate.target_language_name.as_deref(), Some("Japanese"));

        let summary = SonaCoreFacade::summarize_transcript_request_from_json(
            r#"{
                "taskId": "summary-1",
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.openai.com",
                    "apiKey": "secret",
                    "model": "gpt-4o-mini"
                },
                "template": {
                    "id": "general",
                    "name": "General",
                    "instructions": "Summarize."
                },
                "segments": [{
                    "id": "s1",
                    "text": "hello",
                    "start": 0.0,
                    "end": 1.5,
                    "isFinal": true
                }],
                "chunkCharBudget": 1200
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(summary.template.id, "general");
        assert_eq!(summary.segments[0].end, 1.5);
        assert_eq!(summary.chunk_char_budget, Some(1200));
    }

    #[test]
    fn facade_validates_llm_request_json_with_core_rules() {
        let config_error = SonaCoreFacade::validate_llm_config_json(
            r#"{
                "provider": "open_ai",
                "baseUrl": "https://api.openai.com",
                "apiKey": "secret",
                "model": "  "
            }"#
            .to_string(),
        )
        .unwrap_err();
        assert_eq!(config_error.to_string(), "Model name cannot be empty");

        let generate_error = SonaCoreFacade::validate_llm_generate_request_json(
            r#"{
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.openai.com",
                    "apiKey": "secret",
                    "model": "gpt-4o-mini"
                },
                "input": "  "
            }"#
            .to_string(),
        )
        .unwrap_err();
        assert_eq!(generate_error.to_string(), "Input cannot be empty");

        let polish_error = SonaCoreFacade::validate_polish_segments_request_json(
            r#"{
                "taskId": "polish-1",
                "config": {
                    "provider": "google_translate_free",
                    "baseUrl": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": "",
                    "model": "translate"
                },
                "segments": []
            }"#
            .to_string(),
        )
        .unwrap_err();
        assert_eq!(
            polish_error.to_string(),
            "Google Translate does not support transcript polishing"
        );

        let translate_error = SonaCoreFacade::validate_translate_segments_request_json(
            r#"{
                "taskId": "translate-1",
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.openai.com",
                    "apiKey": "secret",
                    "model": "gpt-4o-mini"
                },
                "segments": [],
                "targetLanguage": "  "
            }"#
            .to_string(),
        )
        .unwrap_err();
        assert_eq!(
            translate_error.to_string(),
            "Target language cannot be empty"
        );

        let summary_error = SonaCoreFacade::validate_summarize_transcript_request_json(
            r#"{
                "taskId": "summary-1",
                "config": {
                    "provider": "google_translate",
                    "baseUrl": "https://translation.googleapis.com/language/translate/v2",
                    "apiKey": "secret",
                    "model": "translate"
                },
                "template": {
                    "id": "general",
                    "name": "General",
                    "instructions": "Summarize."
                },
                "segments": []
            }"#
            .to_string(),
        )
        .unwrap_err();
        assert_eq!(
            summary_error.to_string(),
            "Google Translate does not support transcript summaries"
        );

        SonaCoreFacade::validate_translate_segments_request_json(
            r#"{
                "taskId": "translate-ok",
                "config": {
                    "provider": "google_translate_free",
                    "baseUrl": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": "",
                    "model": "translate"
                },
                "segments": [{"id": "s1", "text": "hello"}],
                "targetLanguage": "ja"
            }"#
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn facade_exposes_transcript_llm_job_helpers_for_mobile() {
        let segments_json = r#"[
            {
                "id": "s1",
                "text": "Hello",
                "start": 0.0,
                "end": 1.5,
                "isFinal": true,
                "speaker": {
                    "id": "speaker-a",
                    "label": "Alice",
                    "kind": "identified",
                    "score": 0.91
                }
            },
            {
                "id": "s2",
                "text": "world",
                "start": 1.5,
                "end": 2.0,
                "isFinal": true,
                "translation": "old"
            }
        ]"#
        .to_string();

        let segment_inputs =
            SonaCoreFacade::llm_segment_inputs_from_transcript_json(segments_json.clone()).unwrap();
        assert_eq!(segment_inputs[0].id, "s1");
        assert_eq!(segment_inputs[0].text, "Hello");

        let summary_inputs =
            SonaCoreFacade::summary_segment_inputs_from_transcript_json(segments_json.clone())
                .unwrap();
        assert_eq!(summary_inputs[0].text, "Alice: Hello");
        assert_eq!(summary_inputs[0].start, 0.0);
        assert_eq!(summary_inputs[0].end, 1.5);
        assert!(summary_inputs[0].is_final);

        let translated_json = SonaCoreFacade::merge_translated_items_into_transcript_json(
            segments_json.clone(),
            r#"[{"id":"s1","translation":"konnichiwa"}]"#.to_string(),
        )
        .unwrap();
        let translated: serde_json::Value = serde_json::from_str(&translated_json).unwrap();
        assert_eq!(
            translated[0]["translation"],
            serde_json::json!("konnichiwa")
        );
        assert_eq!(
            translated[0]["speaker"]["label"],
            serde_json::json!("Alice")
        );
        assert_eq!(translated[1]["translation"], serde_json::json!("old"));

        let polished_json = SonaCoreFacade::merge_polished_items_into_transcript_json(
            segments_json.clone(),
            r#"[{"id":"s2","text":"World."}]"#.to_string(),
        )
        .unwrap();
        let polished: serde_json::Value = serde_json::from_str(&polished_json).unwrap();
        assert_eq!(polished[1]["text"], serde_json::json!("World."));
        assert_eq!(polished[1]["translation"], serde_json::json!("old"));

        assert_eq!(
            SonaCoreFacade::summary_source_fingerprint_from_transcript_json(segments_json).unwrap(),
            "s1:Hello:0:1.5:true:speaker-a:Alice:identified:0.91|s2:world:1.5:2:true::::"
        );
    }

    #[test]
    fn facade_exposes_llm_prompt_and_chunk_parsing_helpers_for_mobile() {
        let segment_inputs_json = r#"[
            {"id":"s1","text":"hello world"},
            {"id":"s2","text":"next step"}
        ]"#
        .to_string();

        let polish_prompt = SonaCoreFacade::build_polish_prompt_json(
            segment_inputs_json.clone(),
            Some("product review".to_string()),
            Some("Sona".to_string()),
        )
        .unwrap();
        assert!(polish_prompt.contains("[User Context]"));
        assert!(polish_prompt.contains("product review"));
        assert!(polish_prompt.contains("[User Keywords]"));
        assert!(polish_prompt.contains("Sona"));
        assert!(polish_prompt.contains("\"id\":\"s1\""));
        assert!(polish_prompt.contains("Output newline-delimited JSON"));

        let translate_prompt = SonaCoreFacade::build_translate_prompt_json(
            segment_inputs_json.clone(),
            "es".to_string(),
            Some("Spanish".to_string()),
        )
        .unwrap();
        assert!(translate_prompt.contains("into Spanish"));
        assert!(translate_prompt.contains("\"id\":\"s2\""));
        assert!(translate_prompt.contains("replace 'text' with 'translation'"));

        let summary_prompt = SonaCoreFacade::build_summary_chunk_prompt_json(
            r#"{
                "id":"meeting",
                "name":"Meeting",
                "instructions":"- Decisions\n- Actions"
            }"#
            .to_string(),
            r#"[
                {
                    "id":"s1",
                    "text":"Alice: hello",
                    "start":1.0,
                    "end":3.0,
                    "isFinal":true
                }
            ]"#
            .to_string(),
            2,
            3,
        )
        .unwrap();
        assert!(summary_prompt.contains("Meeting"));
        assert!(summary_prompt.contains("chunk 2 of 3"));
        assert!(summary_prompt.contains("[00:00:01 - 00:00:03] Alice: hello"));

        let finalize_prompt = SonaCoreFacade::build_summary_finalize_prompt_json(
            r#"{
                "id":"meeting",
                "name":"Meeting",
                "instructions":"- Decisions\n- Actions"
            }"#
            .to_string(),
            vec!["First summary".to_string(), "Second summary".to_string()],
        )
        .unwrap();
        assert!(finalize_prompt.contains("[Chunk 1]"));
        assert!(finalize_prompt.contains("First summary"));
        assert!(finalize_prompt.contains("[Chunk 2]"));

        let polished = SonaCoreFacade::parse_polish_chunk_json(
            "{\"id\":\"s1\",\"text\":\"Hello world.\"}\n{\"id\":\"s2\",\"text\":\"Next step.\"}"
                .to_string(),
            segment_inputs_json.clone(),
            1,
        )
        .unwrap();
        assert_eq!(
            polished,
            vec![
                FfiPolishedSegment {
                    id: "s1".to_string(),
                    text: "Hello world.".to_string(),
                },
                FfiPolishedSegment {
                    id: "s2".to_string(),
                    text: "Next step.".to_string(),
                },
            ]
        );

        let translated = SonaCoreFacade::parse_translate_chunk_json(
            r#"[
                {"id":"s1","translation":"Hola mundo."},
                {"id":"s2","translation":"Siguiente paso."}
            ]"#
            .to_string(),
            segment_inputs_json,
            2,
        )
        .unwrap();
        assert_eq!(translated[0].id, "s1");
        assert_eq!(translated[0].translation, "Hola mundo.");

        let error = SonaCoreFacade::parse_translate_chunk_json(
            r#"[{"id":"wrong","translation":"Hola"}]"#.to_string(),
            r#"[{"id":"s1","text":"hello"}]"#.to_string(),
            7,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "translate chunk 7 failed: segment 1 expected id 's1' but received 'wrong'"
        );
    }

    #[test]
    fn facade_exposes_llm_prompt_chunk_planning_helpers_for_mobile() {
        let segment_inputs_json = r#"[
            {"id":"s1","text":"first"},
            {"id":"s2","text":"second"},
            {"id":"s3","text":"third"}
        ]"#
        .to_string();

        let polish_chunks = SonaCoreFacade::plan_polish_prompt_chunks_json(
            segment_inputs_json.clone(),
            Some("planning context".to_string()),
            None,
            Some(2),
            None,
        )
        .unwrap();
        assert_eq!(polish_chunks.len(), 2);
        assert_eq!(polish_chunks[0].start, 0);
        assert_eq!(polish_chunks[0].end, 2);
        assert_eq!(polish_chunks[0].chunk_number, 1);
        assert_eq!(polish_chunks[0].total_chunks, 2);
        assert!(polish_chunks[0].prompt.contains("planning context"));
        assert!(polish_chunks[0].prompt.contains("\"id\":\"s1\""));
        assert!(polish_chunks[0].prompt.contains("\"id\":\"s2\""));
        assert!(!polish_chunks[0].prompt.contains("\"id\":\"s3\""));
        assert_eq!(polish_chunks[1].start, 2);
        assert_eq!(polish_chunks[1].end, 3);

        let translate_chunks = SonaCoreFacade::plan_translate_prompt_chunks_json(
            segment_inputs_json,
            "es".to_string(),
            Some("Spanish".to_string()),
            Some(1),
            None,
        )
        .unwrap();
        assert_eq!(translate_chunks.len(), 3);
        assert_eq!(translate_chunks[2].chunk_number, 3);
        assert_eq!(translate_chunks[2].total_chunks, 3);
        assert!(translate_chunks[2].prompt.contains("into Spanish"));
        assert!(translate_chunks[2].prompt.contains("\"id\":\"s3\""));

        let long_text = "a".repeat(1300);
        let summary_segments_json = format!(
            r#"[
                {{
                    "id":"s1",
                    "text":"{}",
                    "start":0.0,
                    "end":2.0,
                    "isFinal":true
                }},
                {{
                    "id":"s2",
                    "text":"{}",
                    "start":2.0,
                    "end":4.0,
                    "isFinal":true
                }}
            ]"#,
            long_text, long_text
        );

        let summary_chunks = SonaCoreFacade::plan_summary_prompt_chunks_json(
            r#"{
                "id":"meeting",
                "name":"Meeting",
                "instructions":"- Decisions\n- Actions"
            }"#
            .to_string(),
            summary_segments_json,
            Some(1200),
        )
        .unwrap();
        assert_eq!(summary_chunks.len(), 2);
        assert_eq!(summary_chunks[0].start, 0);
        assert_eq!(summary_chunks[0].end, 1);
        assert_eq!(summary_chunks[0].chunk_number, 1);
        assert_eq!(summary_chunks[0].total_chunks, 2);
        assert!(summary_chunks[0].prompt.contains("chunk 1 of 2"));
        assert_eq!(summary_chunks[1].start, 1);
        assert_eq!(summary_chunks[1].end, 2);
        assert!(summary_chunks[1].prompt.contains("chunk 2 of 2"));
    }

    #[test]
    fn facade_exposes_binding_safe_preset_model_records() {
        let models = SonaCoreFacade::preset_models();

        let vad = models
            .iter()
            .find(|model| model.id == DEFAULT_SILERO_VAD_MODEL_ID)
            .expect("VAD preset should be exported");

        assert_eq!(vad.model_type, "vad");
        assert_eq!(vad.engine, "sherpa-onnx");
        assert!(
            vad.filename
                .as_deref()
                .unwrap_or_default()
                .ends_with(".onnx")
        );
        assert!(vad.modes.is_empty());
        assert!(vad.rules.requires_vad);
        assert_eq!(vad.rules.timestamp_support_hint, None);
    }

    #[test]
    fn facade_exposes_binding_safe_model_catalog_snapshot() {
        let snapshot = SonaCoreFacade::model_catalog_snapshot(
            "C:/models".to_string(),
            vec![
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
                DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            ],
        );

        let installed_asr = snapshot
            .models
            .iter()
            .find(|model| model.id == "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
            .expect("installed ASR model should be exported");
        assert!(installed_asr.is_installed);
        assert_eq!(installed_asr.rules.timestamp_support_hint, None);
        assert!(installed_asr.install_path.ends_with("2024-07-17"));

        let streaming_option = snapshot
            .selection_options
            .streaming
            .iter()
            .find(|option| option.id == installed_asr.id)
            .expect("installed ASR model should be selectable for streaming");
        assert!(streaming_option.is_installed);

        let path_entry = snapshot
            .model_path_by_id
            .iter()
            .find(|entry| entry.id == installed_asr.id)
            .expect("model path index should be exported");
        assert_eq!(path_entry.path, installed_asr.install_path);

        let normalized_path_entry = snapshot
            .model_id_by_normalized_path
            .iter()
            .find(|entry| entry.id == installed_asr.id)
            .expect("normalized path reverse index should be exported");
        assert_eq!(
            normalized_path_entry.normalized_path,
            installed_asr.install_path.replace('\\', "/").to_lowercase()
        );

        let token = snapshot
            .path_match_tokens
            .iter()
            .find(|token| token.id == installed_asr.id)
            .expect("path match token should be exported");
        assert_eq!(token.token, installed_asr.id.to_lowercase());

        let asr_section = snapshot
            .sections
            .iter()
            .find(|section| section.section_type == FfiModelCatalogSectionType::Asr)
            .expect("ASR section should be exported");
        let sensevoice_group = asr_section
            .groups
            .iter()
            .find(|group| group.key == "sensevoice")
            .expect("SenseVoice models should stay grouped by core");
        assert!(
            sensevoice_group
                .models
                .iter()
                .any(|model| model.id == installed_asr.id)
        );

        let dependencies = snapshot
            .dependency_requests_by_model_id
            .iter()
            .find(|entry| entry.model_id == installed_asr.id)
            .expect("recognition model dependencies should be exported");
        assert_eq!(dependencies.requests.len(), 1);
        assert_eq!(
            dependencies.requests[0].config_key,
            FfiModelDependencyConfigKey::VadModelPath
        );
        assert_eq!(
            dependencies.requests[0].model_id,
            DEFAULT_SILERO_VAD_MODEL_ID
        );
        assert!(dependencies.requests[0].is_installed);

        assert_eq!(
            snapshot.restore_defaults.streaming_model_path,
            Some(installed_asr.install_path.clone())
        );
        assert_eq!(
            snapshot.restore_defaults.vad_model_path,
            snapshot
                .models
                .iter()
                .find(|model| model.id == DEFAULT_SILERO_VAD_MODEL_ID)
                .map(|model| model.install_path.clone())
        );
    }

    #[test]
    fn facade_exposes_default_config_json_for_mobile_bootstrap() {
        let config: serde_json::Value =
            serde_json::from_str(&SonaCoreFacade::default_config_json()).unwrap();

        assert_eq!(config["configVersion"], 7);
        assert_eq!(
            config["asr"]["selections"]["batch"]["mode"],
            serde_json::json!("batch")
        );
        assert_eq!(
            config["llmSettings"]["activeProvider"],
            serde_json::json!("google_translate_free")
        );
    }

    #[test]
    fn facade_migrates_config_json_and_resolves_effective_config_json() {
        let migrated = SonaCoreFacade::migrate_app_config_json(
            Some(
                r#"{
                    "configVersion": 6,
                    "summaryEnabled": null,
                    "summaryTemplateId": "meeting",
                    "summaryCustomTemplates": [],
                    "polishPresetId": "meeting",
                    "polishCustomPresets": [],
                    "polishKeywordSets": [],
                    "speakerProfiles": [],
                    "speakerSegmentationModelPath": "",
                    "speakerEmbeddingModelPath": "",
                    "logLevel": "info",
                    "llmSettings": {
                        "activeProvider": "google_translate_free",
                        "providers": {
                            "google_translate_free": {
                                "apiHost": "https://translate.googleapis.com/translate_a/single",
                                "apiKey": ""
                            }
                        },
                        "models": {},
                        "modelOrder": [],
                        "selections": {}
                    }
                }"#
                .to_string(),
            ),
            None,
            "Default Rules".to_string(),
        )
        .unwrap();
        let migrated_config: serde_json::Value =
            serde_json::from_str(&migrated.config_json).unwrap();

        assert!(migrated.migrated);
        assert_eq!(migrated_config["configVersion"], 7);
        assert_eq!(migrated_config["summaryEnabled"], true);

        let effective = SonaCoreFacade::resolve_effective_config_json(
            migrated.config_json,
            Some(
                r#"{
                    "defaults": {
                        "summaryTemplateId": "meeting",
                        "translationLanguage": "ja",
                        "polishPresetId": "meeting"
                    }
                }"#
                .to_string(),
            ),
        )
        .unwrap();
        let effective_config: serde_json::Value = serde_json::from_str(&effective).unwrap();

        assert_eq!(effective_config["summaryTemplateId"], "meeting");
        assert_eq!(effective_config["translationLanguage"], "ja");
        assert_eq!(effective_config["polishPresetId"], "meeting");
    }

    #[test]
    fn facade_rejects_invalid_config_json() {
        let error = SonaCoreFacade::migrate_app_config_json(
            Some("{bad-json".to_string()),
            None,
            "Default Rules".to_string(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("Invalid saved config JSON"));
    }

    #[test]
    fn facade_resolves_model_catalog_selected_ids_for_mobile_paths() {
        let selected = SonaCoreFacade::model_catalog_selected_ids(
            "C:/models".to_string(),
            vec![
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
                "3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx".to_string(),
            ],
            FfiModelSelectionPaths {
                streaming_model_path:
                    "C:/models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
                batch_model_path: "D:\\portable\\sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25"
                    .to_string(),
                speaker_segmentation_model_path: String::new(),
                speaker_embedding_model_path:
                    "D:/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx"
                        .to_string(),
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

    #[test]
    fn facade_resolves_model_download_plan_for_mobile() {
        let download = SonaCoreFacade::resolve_model_download(
            "sherpa-onnx-funasr-nano-int8-2025-12-30".to_string(),
            "C:/models".to_string(),
        )
        .unwrap();

        assert_eq!(download.model.id, "sherpa-onnx-funasr-nano-int8-2025-12-30");
        assert_eq!(download.models_dir, "C:/models");
        assert!(
            download
                .download_path
                .ends_with("sherpa-onnx-funasr-nano-int8-2025-12-30.tar.bz2")
        );
        assert!(
            download
                .install_path
                .ends_with("sherpa-onnx-funasr-nano-int8-2025-12-30")
        );
        assert_eq!(
            download.required_companions.vad_model_id.as_deref(),
            Some(DEFAULT_SILERO_VAD_MODEL_ID)
        );
        assert_eq!(
            download.required_companions.punctuation_model_id.as_deref(),
            Some(DEFAULT_PUNCTUATION_MODEL_ID)
        );
    }

    #[test]
    fn facade_rejects_unknown_model_download_id() {
        let error = SonaCoreFacade::resolve_model_download(
            "missing-model".to_string(),
            "C:/models".to_string(),
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "Unknown model id: missing-model");
    }
}
