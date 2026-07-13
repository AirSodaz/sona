use crate::{
    FfiAsrStreamingObserver, FfiAsrStreamingSession, FfiBatchSegmentationMode,
    FfiConfigMigrationResult, FfiLlmConfig, FfiLlmPromptChunk, FfiLlmProvider, FfiLlmSegmentInput,
    FfiModelCatalogSelectedIds, FfiModelCatalogSnapshot, FfiModelSelectionPaths,
    FfiOnlineAsrProvider, FfiOnlineAsrProviderRequest, FfiPolishSegmentsRequest,
    FfiPolishedSegment, FfiPresetModel, FfiResolvedModelDownload, FfiRuntimePathStatus,
    FfiSummarizeTranscriptRequest, FfiSummarySegmentInput, FfiTranslateSegmentsRequest,
    FfiTranslatedSegment, FfiVolcengineDoubaoAsrConfig, SonaCoreBindingResult,
    app_config_repository_bridge, asr_bridge, asr_streaming_bridge, automation_bridge,
    config_bridge, dashboard_bridge, diagnostics_bridge, export_bridge, history_mutation_bridge,
    history_query_bridge, llm_bridge, model_bridge, project_bridge, recovery_bridge,
    runtime_bridge, storage_usage_bridge, task_ledger_bridge,
};
use std::sync::Arc;

/// Rust facade used by tests and by the top-level UniFFI exports.
pub struct SonaCoreFacade;

impl SonaCoreFacade {
    pub fn load_project_repository_state_json(
        app_data_dir: String,
    ) -> SonaCoreBindingResult<String> {
        project_bridge::load_project_repository_state_json(app_data_dir)
    }

    pub fn replace_projects_json(
        app_data_dir: String,
        projects_json: String,
    ) -> SonaCoreBindingResult<()> {
        project_bridge::replace_projects_json(app_data_dir, projects_json)
    }

    pub fn create_project_json(
        app_data_dir: String,
        input_json: String,
    ) -> SonaCoreBindingResult<String> {
        project_bridge::create_project_json(app_data_dir, input_json)
    }

    pub fn update_project_json(
        app_data_dir: String,
        project_id: String,
        updates_json: String,
    ) -> SonaCoreBindingResult<String> {
        project_bridge::update_project_json(app_data_dir, project_id, updates_json)
    }

    pub fn delete_project(app_data_dir: String, project_id: String) -> SonaCoreBindingResult<()> {
        project_bridge::delete_project(app_data_dir, project_id)
    }

    pub fn reorder_projects_json(
        app_data_dir: String,
        project_ids_json: String,
    ) -> SonaCoreBindingResult<String> {
        project_bridge::reorder_projects_json(app_data_dir, project_ids_json)
    }

    pub fn set_active_project_id(
        app_data_dir: String,
        project_id: Option<String>,
    ) -> SonaCoreBindingResult<()> {
        project_bridge::set_active_project_id(app_data_dir, project_id)
    }

    pub fn load_recovery_snapshot_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
        recovery_bridge::load_recovery_snapshot_json(app_data_dir)
    }

    pub fn save_recovery_snapshot_json(
        app_data_dir: String,
        items_json: String,
    ) -> SonaCoreBindingResult<String> {
        recovery_bridge::save_recovery_snapshot_json(app_data_dir, items_json)
    }

    pub fn persist_recovery_queue_snapshot_json(
        app_data_dir: String,
        queue_items_json: String,
        resolved_ids: Vec<String>,
    ) -> SonaCoreBindingResult<String> {
        recovery_bridge::persist_recovery_queue_snapshot_json(
            app_data_dir,
            queue_items_json,
            resolved_ids,
        )
    }

    pub fn load_task_ledger_snapshot_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::load_task_ledger_snapshot_json(app_data_dir)
    }

    pub fn upsert_task_ledger_record_json(
        app_data_dir: String,
        record_json: String,
    ) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::upsert_task_ledger_record_json(app_data_dir, record_json)
    }

    pub fn patch_task_ledger_record_json(
        app_data_dir: String,
        id: String,
        patch_json: String,
    ) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::patch_task_ledger_record_json(app_data_dir, id, patch_json)
    }

    pub fn remove_task_ledger_record_json(
        app_data_dir: String,
        id: String,
    ) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::remove_task_ledger_record_json(app_data_dir, id)
    }

    pub fn clear_resolved_task_ledger_records_json(
        app_data_dir: String,
    ) -> SonaCoreBindingResult<String> {
        task_ledger_bridge::clear_resolved_task_ledger_records_json(app_data_dir)
    }

    pub fn load_automation_repository_state_json(
        app_data_dir: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::load_automation_repository_state_json(app_data_dir)
    }

    pub fn replace_automation_rules_json(
        app_data_dir: String,
        rules_json: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::replace_automation_rules_json(app_data_dir, rules_json)
    }

    pub fn replace_automation_processed_entries_json(
        app_data_dir: String,
        entries_json: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::replace_automation_processed_entries_json(app_data_dir, entries_json)
    }

    pub fn replace_automation_repository_state_json(
        app_data_dir: String,
        state_json: String,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::replace_automation_repository_state_json(app_data_dir, state_json)
    }

    pub fn validate_automation_rule_activation_json(
        rule_json: String,
        global_config_json: String,
        project_json: Option<String>,
    ) -> SonaCoreBindingResult<String> {
        automation_bridge::validate_automation_rule_activation_json(
            rule_json,
            global_config_json,
            project_json,
        )
    }

    pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
        runtime_bridge::normalize_export_format(value)
    }

    pub async fn export_transcript_file_json(input_json: String) -> SonaCoreBindingResult<String> {
        export_bridge::export_transcript_file_json(input_json).await
    }

    pub async fn list_history_items_json(
        app_data_dir: String,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::list_history_items_json(app_data_dir, limit, offset).await
    }

    pub async fn query_history_workspace_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::query_history_workspace_json(app_data_dir, request_json).await
    }

    pub async fn load_history_transcript_json(
        app_data_dir: String,
        history_id: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::load_history_transcript_json(app_data_dir, history_id).await
    }

    pub async fn list_history_transcript_snapshots_json(
        app_data_dir: String,
        history_id: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::list_history_transcript_snapshots_json(app_data_dir, history_id).await
    }

    pub async fn load_history_transcript_snapshot_json(
        app_data_dir: String,
        history_id: String,
        snapshot_id: String,
    ) -> SonaCoreBindingResult<String> {
        history_query_bridge::load_history_transcript_snapshot_json(
            app_data_dir,
            history_id,
            snapshot_id,
        )
        .await
    }

    pub async fn create_history_live_draft_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::create_history_live_draft_json(app_data_dir, request_json).await
    }

    pub async fn complete_history_live_draft_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::complete_history_live_draft_json(app_data_dir, request_json).await
    }

    pub async fn save_history_recording_json(
        app_data_dir: String,
        request_json: String,
        audio_bytes: Option<Vec<u8>>,
        native_audio_path: Option<String>,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::save_history_recording_json(
            app_data_dir,
            request_json,
            audio_bytes,
            native_audio_path,
        )
        .await
    }

    pub async fn save_history_imported_file_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::save_history_imported_file_json(app_data_dir, request_json).await
    }

    pub async fn delete_history_items_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::delete_history_items_json(app_data_dir, request_json).await
    }

    pub async fn update_history_transcript_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_transcript_json(app_data_dir, request_json).await
    }

    pub async fn create_history_transcript_snapshot_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::create_history_transcript_snapshot_json(app_data_dir, request_json)
            .await
    }

    pub async fn update_history_item_meta_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_item_meta_json(app_data_dir, request_json).await
    }

    pub async fn update_history_project_assignments_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::update_history_project_assignments_json(app_data_dir, request_json)
            .await
    }

    pub async fn reassign_history_project_json(
        app_data_dir: String,
        request_json: String,
    ) -> SonaCoreBindingResult<String> {
        history_mutation_bridge::reassign_history_project_json(app_data_dir, request_json).await
    }

    pub fn default_vad_model_id() -> String {
        model_bridge::default_vad_model_id()
    }

    pub fn default_punctuation_model_id() -> String {
        model_bridge::default_punctuation_model_id()
    }

    pub fn preset_model_name(model_id: String) -> Option<String> {
        model_bridge::preset_model_name(model_id)
    }

    pub fn preset_models() -> Vec<FfiPresetModel> {
        model_bridge::preset_models()
    }

    pub fn model_catalog_snapshot(
        models_dir: String,
        installed_model_ids: Vec<String>,
    ) -> FfiModelCatalogSnapshot {
        model_bridge::model_catalog_snapshot(models_dir, installed_model_ids)
    }

    pub fn model_catalog_selected_ids(
        models_dir: String,
        installed_model_ids: Vec<String>,
        paths: FfiModelSelectionPaths,
    ) -> FfiModelCatalogSelectedIds {
        model_bridge::model_catalog_selected_ids(models_dir, installed_model_ids, paths)
    }

    pub fn resolve_model_download(
        model_id: String,
        models_dir: String,
    ) -> SonaCoreBindingResult<FfiResolvedModelDownload> {
        model_bridge::resolve_model_download(model_id, models_dir)
    }

    pub fn resolve_gpu_acceleration(
        value: Option<String>,
    ) -> SonaCoreBindingResult<Option<String>> {
        model_bridge::resolve_gpu_acceleration(value)
    }

    pub fn default_config_json() -> String {
        config_bridge::default_config_json()
    }

    pub fn migrate_app_config_json(
        saved_config_json: Option<String>,
        legacy_config_json: Option<String>,
        default_rule_set_name: String,
    ) -> SonaCoreBindingResult<FfiConfigMigrationResult> {
        config_bridge::migrate_app_config_json(
            saved_config_json,
            legacy_config_json,
            default_rule_set_name,
        )
    }

    pub fn resolve_effective_config_json(
        global_config_json: String,
        project_json: Option<String>,
    ) -> SonaCoreBindingResult<String> {
        config_bridge::resolve_effective_config_json(global_config_json, project_json)
    }

    pub fn load_app_config_json(app_data_dir: String) -> SonaCoreBindingResult<Option<String>> {
        app_config_repository_bridge::load_app_config_json(app_data_dir)
    }

    pub fn save_app_config_json(
        app_data_dir: String,
        config_json: String,
    ) -> SonaCoreBindingResult<()> {
        app_config_repository_bridge::save_app_config_json(app_data_dir, config_json)
    }

    pub fn get_app_setting_json(
        app_data_dir: String,
        key: String,
    ) -> SonaCoreBindingResult<Option<String>> {
        app_config_repository_bridge::get_app_setting_json(app_data_dir, key)
    }

    pub fn set_app_setting_json(
        app_data_dir: String,
        key: String,
        value_json: String,
    ) -> SonaCoreBindingResult<()> {
        app_config_repository_bridge::set_app_setting_json(app_data_dir, key, value_json)
    }

    pub async fn load_dashboard_snapshot_json(
        app_data_dir: String,
        deep: bool,
    ) -> SonaCoreBindingResult<String> {
        dashboard_bridge::load_dashboard_snapshot_json(app_data_dir, deep).await
    }

    pub async fn load_diagnostics_snapshot_json(
        app_data_dir: String,
        input_json: String,
    ) -> SonaCoreBindingResult<String> {
        diagnostics_bridge::load_diagnostics_snapshot_json(app_data_dir, input_json).await
    }

    pub async fn load_storage_usage_snapshot_json(
        app_data_dir: String,
    ) -> SonaCoreBindingResult<String> {
        storage_usage_bridge::load_storage_usage_snapshot_json(app_data_dir).await
    }

    pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
        runtime_bridge::runtime_path_status(path)
    }

    pub fn create_online_asr_streaming_session(
        instance_id: String,
        request_json: String,
        observer: Arc<dyn FfiAsrStreamingObserver>,
    ) -> SonaCoreBindingResult<Arc<FfiAsrStreamingSession>> {
        asr_streaming_bridge::create_online_asr_streaming_session(
            instance_id,
            request_json,
            observer,
        )
    }

    pub fn default_batch_segmentation_mode() -> FfiBatchSegmentationMode {
        asr_bridge::default_batch_segmentation_mode()
    }

    pub fn online_asr_providers() -> Vec<FfiOnlineAsrProvider> {
        asr_bridge::online_asr_providers()
    }

    pub fn find_online_asr_provider(provider_id: String) -> Option<FfiOnlineAsrProvider> {
        asr_bridge::find_online_asr_provider(provider_id)
    }

    pub fn online_asr_provider_request(
        provider_id: String,
        profile_id: String,
        config_json: String,
    ) -> SonaCoreBindingResult<FfiOnlineAsrProviderRequest> {
        asr_bridge::online_asr_provider_request(provider_id, profile_id, config_json)
    }

    pub fn volcengine_doubao_asr_config_from_json(
        config_json: String,
    ) -> SonaCoreBindingResult<FfiVolcengineDoubaoAsrConfig> {
        asr_bridge::volcengine_doubao_asr_config_from_json(config_json)
    }

    pub fn llm_providers() -> Vec<FfiLlmProvider> {
        llm_bridge::llm_providers()
    }

    pub fn find_llm_provider_by_id_or_alias(id_or_alias: String) -> Option<FfiLlmProvider> {
        llm_bridge::find_llm_provider_by_id_or_alias(id_or_alias)
    }

    pub fn llm_config_from_json(config_json: String) -> SonaCoreBindingResult<FfiLlmConfig> {
        llm_bridge::llm_config_from_json(config_json)
    }

    pub fn validate_llm_config_json(config_json: String) -> SonaCoreBindingResult<()> {
        llm_bridge::validate_llm_config_json(config_json)
    }

    pub fn validate_llm_generate_request_json(request_json: String) -> SonaCoreBindingResult<()> {
        llm_bridge::validate_llm_generate_request_json(request_json)
    }

    pub fn validate_polish_segments_request_json(
        request_json: String,
    ) -> SonaCoreBindingResult<()> {
        llm_bridge::validate_polish_segments_request_json(request_json)
    }

    pub fn validate_translate_segments_request_json(
        request_json: String,
    ) -> SonaCoreBindingResult<()> {
        llm_bridge::validate_translate_segments_request_json(request_json)
    }

    pub fn validate_summarize_transcript_request_json(
        request_json: String,
    ) -> SonaCoreBindingResult<()> {
        llm_bridge::validate_summarize_transcript_request_json(request_json)
    }

    pub fn llm_segment_inputs_from_transcript_json(
        segments_json: String,
    ) -> SonaCoreBindingResult<Vec<FfiLlmSegmentInput>> {
        llm_bridge::llm_segment_inputs_from_transcript_json(segments_json)
    }

    pub fn summary_segment_inputs_from_transcript_json(
        segments_json: String,
    ) -> SonaCoreBindingResult<Vec<FfiSummarySegmentInput>> {
        llm_bridge::summary_segment_inputs_from_transcript_json(segments_json)
    }

    pub fn merge_translated_items_into_transcript_json(
        segments_json: String,
        items_json: String,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::merge_translated_items_into_transcript_json(segments_json, items_json)
    }

    pub fn merge_polished_items_into_transcript_json(
        segments_json: String,
        items_json: String,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::merge_polished_items_into_transcript_json(segments_json, items_json)
    }

    pub fn summary_source_fingerprint_from_transcript_json(
        segments_json: String,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::summary_source_fingerprint_from_transcript_json(segments_json)
    }

    pub fn build_polish_prompt_json(
        segments_json: String,
        context: Option<String>,
        keywords: Option<String>,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::build_polish_prompt_json(segments_json, context, keywords)
    }

    pub fn build_translate_prompt_json(
        segments_json: String,
        target_language: String,
        target_language_name: Option<String>,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::build_translate_prompt_json(
            segments_json,
            target_language,
            target_language_name,
        )
    }

    pub fn build_summary_chunk_prompt_json(
        template_json: String,
        segments_json: String,
        chunk_number: u64,
        total_chunks: u64,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::build_summary_chunk_prompt_json(
            template_json,
            segments_json,
            chunk_number,
            total_chunks,
        )
    }

    pub fn build_summary_finalize_prompt_json(
        template_json: String,
        partial_summaries: Vec<String>,
    ) -> SonaCoreBindingResult<String> {
        llm_bridge::build_summary_finalize_prompt_json(template_json, partial_summaries)
    }

    pub fn plan_polish_prompt_chunks_json(
        segments_json: String,
        context: Option<String>,
        keywords: Option<String>,
        chunk_size: Option<u64>,
        prompt_char_budget: Option<u64>,
    ) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
        llm_bridge::plan_polish_prompt_chunks_json(
            segments_json,
            context,
            keywords,
            chunk_size,
            prompt_char_budget,
        )
    }

    pub fn plan_translate_prompt_chunks_json(
        segments_json: String,
        target_language: String,
        target_language_name: Option<String>,
        chunk_size: Option<u64>,
        prompt_char_budget: Option<u64>,
    ) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
        llm_bridge::plan_translate_prompt_chunks_json(
            segments_json,
            target_language,
            target_language_name,
            chunk_size,
            prompt_char_budget,
        )
    }

    pub fn plan_summary_prompt_chunks_json(
        template_json: String,
        segments_json: String,
        chunk_char_budget: Option<u64>,
    ) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
        llm_bridge::plan_summary_prompt_chunks_json(template_json, segments_json, chunk_char_budget)
    }

    pub fn parse_polish_chunk_json(
        response_text: String,
        expected_segments_json: String,
        chunk_number: u64,
    ) -> SonaCoreBindingResult<Vec<FfiPolishedSegment>> {
        llm_bridge::parse_polish_chunk_json(response_text, expected_segments_json, chunk_number)
    }

    pub fn parse_translate_chunk_json(
        response_text: String,
        expected_segments_json: String,
        chunk_number: u64,
    ) -> SonaCoreBindingResult<Vec<FfiTranslatedSegment>> {
        llm_bridge::parse_translate_chunk_json(response_text, expected_segments_json, chunk_number)
    }

    pub fn polish_segments_request_from_json(
        request_json: String,
    ) -> SonaCoreBindingResult<FfiPolishSegmentsRequest> {
        llm_bridge::polish_segments_request_from_json(request_json)
    }

    pub fn translate_segments_request_from_json(
        request_json: String,
    ) -> SonaCoreBindingResult<FfiTranslateSegmentsRequest> {
        llm_bridge::translate_segments_request_from_json(request_json)
    }

    pub fn summarize_transcript_request_from_json(
        request_json: String,
    ) -> SonaCoreBindingResult<FfiSummarizeTranscriptRequest> {
        llm_bridge::summarize_transcript_request_from_json(request_json)
    }
}

#[cfg(test)]
mod task_ledger_tests {
    use super::SonaCoreFacade;
    use serde_json::{Value, json};

    #[test]
    fn facade_loads_and_upserts_task_ledger_json() {
        let dir = tempfile::tempdir().unwrap();
        let app_data_dir = dir.path().to_string_lossy().into_owned();

        let empty = SonaCoreFacade::load_task_ledger_snapshot_json(app_data_dir.clone()).unwrap();
        assert_eq!(empty, r#"{"version":1,"updatedAt":null,"tasks":[]}"#);

        let record = json!({
            "id": "facade-task",
            "kind": "llmPolish",
            "status": "pending",
            "title": "Facade task",
            "progress": 0.0,
            "createdAt": 1,
            "updatedAt": 1,
            "retryable": false,
            "cancelable": true,
            "recoverable": false
        });
        let output =
            SonaCoreFacade::upsert_task_ledger_record_json(app_data_dir, record.to_string())
                .unwrap();
        let output: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(output["tasks"][0]["id"], "facade-task");
    }
}

#[cfg(test)]
mod automation_tests {
    use super::SonaCoreFacade;
    use serde_json::{Value, json};
    use std::fs;

    #[test]
    fn facade_delegates_automation_repository_load() {
        let dir = tempfile::tempdir().unwrap();

        let output = SonaCoreFacade::load_automation_repository_state_json(
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();

        assert_eq!(output, r#"{"rules":[],"processedEntries":[]}"#);
    }

    #[test]
    fn facade_delegates_automation_validation() {
        let dir = tempfile::tempdir().unwrap();
        let watch_directory = dir.path().join("watch");
        let output_directory = dir.path().join("output");
        let model_path = dir.path().join("model.onnx");
        fs::create_dir(&watch_directory).unwrap();
        fs::write(&model_path, b"model").unwrap();

        let output = SonaCoreFacade::validate_automation_rule_activation_json(
            json!({
                "name": "Rule",
                "projectId": "inbox",
                "watchDirectory": watch_directory,
                "exportConfig": {
                    "directory": output_directory,
                    "mode": "original"
                }
            })
            .to_string(),
            json!({"offlineModelPath": model_path}).to_string(),
            None,
        )
        .unwrap();
        let result: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(result["valid"], true);
        assert!(output_directory.is_dir());
    }
}

#[cfg(test)]
mod project_tests {
    use super::SonaCoreFacade;
    use serde_json::json;

    #[test]
    fn facade_delegates_project_load_and_create() {
        let dir = tempfile::tempdir().unwrap();
        let app_data_dir = dir.path().to_string_lossy().into_owned();

        let empty =
            SonaCoreFacade::load_project_repository_state_json(app_data_dir.clone()).unwrap();
        let created = SonaCoreFacade::create_project_json(
            app_data_dir,
            json!({"name":"Facade","defaults":{}}).to_string(),
        )
        .unwrap();

        assert_eq!(empty, r#"{"projects":[],"activeProjectId":null}"#);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&created).unwrap()["name"],
            "Facade"
        );
    }
}
