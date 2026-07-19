use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TauriCommandContract {
    pub command: &'static str,
    pub args: &'static str,
    pub result: &'static str,
}

impl TauriCommandContract {
    const fn new(command: &'static str, args: &'static str, result: &'static str) -> Self {
        Self {
            command,
            args,
            result,
        }
    }
}

const RUST_OWNED_TAURI_COMMAND_CONTRACTS: &[TauriCommandContract] = &[
    TauriCommandContract::new(
        "project_list",
        "{ fallbackEnabledPolishKeywordSetIds?: string[] | null; fallbackEnabledSpeakerProfileIds?: string[] | null }",
        "ProjectRecord_Serialize[]",
    ),
    TauriCommandContract::new(
        "project_save_all",
        "{ projects: ProjectRecord_Deserialize[] }",
        "void",
    ),
    TauriCommandContract::new(
        "project_create",
        "{ name: string; description?: string | null; icon?: string | null; defaults: ProjectDefaultsInput }",
        "ProjectRecord_Serialize",
    ),
    TauriCommandContract::new(
        "project_update",
        "{ projectId: string; updates: ProjectUpdateInput }",
        "ProjectRecord_Serialize | null",
    ),
    TauriCommandContract::new("project_delete", "{ projectId: string }", "void"),
    TauriCommandContract::new(
        "project_reorder",
        "{ projectIds: string[] }",
        "ProjectRecord_Serialize[]",
    ),
    TauriCommandContract::new("project_get_active_id", "undefined", "string | null"),
    TauriCommandContract::new(
        "project_set_active_id",
        "{ projectId: string | null }",
        "void",
    ),
    TauriCommandContract::new(
        "tag_list",
        "{ fallbackEnabledPolishKeywordSetIds?: string[] | null; fallbackEnabledSpeakerProfileIds?: string[] | null }",
        "TagRecord_Serialize[]",
    ),
    TauriCommandContract::new("tag_save_all", "{ tags: TagRecord_Deserialize[] }", "void"),
    TauriCommandContract::new(
        "tag_create",
        "{ name: string; description?: string | null; icon?: string | null; color?: string | null; defaults: TagDefaultsInput }",
        "TagRecord_Serialize",
    ),
    TauriCommandContract::new(
        "tag_update",
        "{ tagId: string; updates: TagUpdateInput }",
        "TagRecord_Serialize | null",
    ),
    TauriCommandContract::new("tag_delete", "{ tagId: string }", "void"),
    TauriCommandContract::new(
        "tag_reorder",
        "{ tagIds: string[] }",
        "TagRecord_Serialize[]",
    ),
    TauriCommandContract::new("tag_get_active_id", "undefined", "string | null"),
    TauriCommandContract::new("tag_set_active_id", "{ tagId: string | null }", "void"),
    TauriCommandContract::new(
        "task_ledger_load_snapshot",
        "undefined",
        "TaskLedgerSnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "task_ledger_upsert_task",
        "{ record: TaskLedgerRecord_Deserialize }",
        "TaskLedgerSnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "task_ledger_patch_task",
        "{ id: string; patch: TaskLedgerPatch_Deserialize }",
        "TaskLedgerSnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "task_ledger_remove_task",
        "{ id: string }",
        "TaskLedgerSnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "task_ledger_clear_resolved",
        "undefined",
        "TaskLedgerSnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "recovery_load_snapshot",
        "undefined",
        "RecoverySnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "recovery_save_snapshot",
        "{ items: RecoveryItemInput_Deserialize[] }",
        "RecoverySnapshot_Serialize",
    ),
    TauriCommandContract::new(
        "recovery_persist_queue_snapshot",
        "{ queueItems: RecoveryItemInput_Deserialize[]; resolvedIds?: string[] | null }",
        "void",
    ),
    TauriCommandContract::new(
        "automation_load_repository_state",
        "undefined",
        "AutomationRepositoryState_Serialize",
    ),
    TauriCommandContract::new(
        "automation_persist_rules",
        "{ rules: AutomationRuleInput_Deserialize[] }",
        "void",
    ),
    TauriCommandContract::new(
        "automation_persist_processed_entries",
        "{ processedEntries: AutomationProcessedInput_Deserialize[] }",
        "void",
    ),
    TauriCommandContract::new(
        "automation_persist_repository_state",
        "{ rules: AutomationRuleInput_Deserialize[]; processedEntries: AutomationProcessedInput_Deserialize[] }",
        "void",
    ),
    TauriCommandContract::new(
        "automation_validate_rule_activation",
        "{ rule: AutomationRule; globalConfig: unknown; tags: unknown[] }",
        "AutomationRuleValidationResult_Serialize",
    ),
    TauriCommandContract::new(
        "replace_automation_runtime_rules",
        "{ rules: AutomationRuntimeRuleConfig[] }",
        "AutomationRuntimeReplaceResult[]",
    ),
    TauriCommandContract::new(
        "scan_automation_runtime_rule",
        "{ rule: AutomationRuntimeRuleConfig }",
        "void",
    ),
    TauriCommandContract::new(
        "collect_automation_runtime_rule_paths",
        "{ rule: AutomationRuntimeRuleConfig; filePaths: string[] }",
        "AutomationRuntimePathCollectionResult[]",
    ),
    TauriCommandContract::new(
        "history_list_items",
        "{ limit?: number | null; offset?: number | null } | undefined",
        "HistoryItemRecord[]",
    ),
    TauriCommandContract::new(
        "history_query_workspace",
        "HistoryWorkspaceQueryRequest",
        "HistoryWorkspaceQueryResult",
    ),
    TauriCommandContract::new(
        "history_create_live_draft",
        "HistoryCreateLiveDraftRequest",
        "LiveRecordingDraftResult",
    ),
    TauriCommandContract::new(
        "history_complete_live_draft",
        "HistoryCompleteLiveDraftRequest_Deserialize",
        "HistoryItemRecord",
    ),
    TauriCommandContract::new(
        "history_save_recording",
        "HistorySaveRecordingRequest_Deserialize",
        "HistoryItemRecord",
    ),
    TauriCommandContract::new(
        "history_save_imported_file",
        "HistorySaveImportedFileRequest_Deserialize",
        "HistoryItemRecord",
    ),
    TauriCommandContract::new("history_delete_items", "HistoryDeleteItemsRequest", "void"),
    TauriCommandContract::new("history_trash_items", "HistoryTrashItemsRequest", "void"),
    TauriCommandContract::new("history_restore_items", "HistoryDeleteItemsRequest", "void"),
    TauriCommandContract::new("history_purge_items", "HistoryDeleteItemsRequest", "void"),
    TauriCommandContract::new(
        "history_load_transcript",
        "{ historyId: string }",
        "TranscriptSegment_Serialize[] | null",
    ),
    TauriCommandContract::new(
        "history_update_transcript",
        "HistoryUpdateTranscriptRequest_Deserialize",
        "HistoryItemRecord",
    ),
    TauriCommandContract::new(
        "history_create_transcript_snapshot",
        "HistoryCreateTranscriptSnapshotRequest_Deserialize",
        "TranscriptSnapshotMetadata",
    ),
    TauriCommandContract::new(
        "history_list_transcript_snapshots",
        "{ historyId: string }",
        "TranscriptSnapshotMetadata[]",
    ),
    TauriCommandContract::new(
        "history_load_transcript_snapshot",
        "{ historyId: string; snapshotId: string }",
        "TranscriptSnapshotRecord_Serialize | null",
    ),
    TauriCommandContract::new(
        "history_build_transcript_diff",
        "{ snapshotSegments: TranscriptSegment_Deserialize[]; currentSegments: TranscriptSegment_Deserialize[] }",
        "TranscriptDiffResult_Serialize",
    ),
    TauriCommandContract::new(
        "history_restore_transcript_diff_rows",
        "{ rows: TranscriptDiffRow_Deserialize[]; selectedRowIds: string[] }",
        "TranscriptSegment_Serialize[]",
    ),
    TauriCommandContract::new(
        "history_update_item_meta",
        "HistoryUpdateItemMetaRequest_Deserialize",
        "void",
    ),
    TauriCommandContract::new(
        "history_update_project_assignments",
        "{ ids: string[]; projectId: string | null }",
        "void",
    ),
    TauriCommandContract::new(
        "history_reassign_project",
        "{ currentProjectId: string; nextProjectId: string | null }",
        "void",
    ),
    TauriCommandContract::new(
        "history_update_tag_assignments",
        "HistoryUpdateTagAssignmentsRequest",
        "void",
    ),
    TauriCommandContract::new(
        "history_replace_tag_assignments",
        "HistoryReplaceTagAssignmentsRequest",
        "void",
    ),
    TauriCommandContract::new(
        "history_load_summary",
        "{ historyId: string }",
        "HistorySummaryPayload_Serialize | null",
    ),
    TauriCommandContract::new(
        "history_save_summary",
        "{ historyId: string; summaryPayload: HistorySummaryPayload_Deserialize }",
        "void",
    ),
    TauriCommandContract::new("history_delete_summary", "{ historyId: string }", "void"),
    TauriCommandContract::new(
        "history_resolve_audio_path",
        "{ historyId: string }",
        "string | null",
    ),
    TauriCommandContract::new(
        "history_preview_audio_cleanup",
        "HistoryAudioCleanupRequest_Deserialize",
        "HistoryAudioCleanupReport",
    ),
    TauriCommandContract::new(
        "history_cleanup_audio",
        "HistoryAudioCleanupRequest_Deserialize",
        "HistoryAudioCleanupReport",
    ),
    TauriCommandContract::new("history_open_folder", "undefined", "void"),
];

pub fn rust_owned_tauri_command_contracts() -> &'static [TauriCommandContract] {
    RUST_OWNED_TAURI_COMMAND_CONTRACTS
}

pub fn render_rust_tauri_command_contract_map() -> String {
    let mut output = String::from("export type RustTauriCommandContractMap = {\n");
    for contract in rust_owned_tauri_command_contracts() {
        writeln!(output, "\t\"{}\": {{", contract.command).expect("writing to String cannot fail");
        writeln!(output, "\t\targs: {};", contract.args).expect("writing to String cannot fail");
        writeln!(output, "\t\tresult: {};", contract.result)
            .expect("writing to String cannot fail");
        output.push_str("\t};\n");
    }
    output.push_str("};\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn tauri_command_contract_registry_is_unique_and_complete_for_the_slice() {
        let contracts = rust_owned_tauri_command_contracts();
        assert_eq!(contracts.len(), 61);
        let names = contracts
            .iter()
            .map(|contract| contract.command)
            .collect::<HashSet<_>>();
        assert_eq!(names.len(), contracts.len());

        for expected in [
            "project_list",
            "tag_list",
            "task_ledger_load_snapshot",
            "recovery_load_snapshot",
            "automation_load_repository_state",
            "replace_automation_runtime_rules",
            "history_list_items",
            "history_open_folder",
        ] {
            assert!(names.contains(expected), "missing {expected}");
        }
    }
}
