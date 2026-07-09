use crate::{
    FfiBatchSegmentationMode, FfiConfigMigrationResult, FfiLlmConfig, FfiLlmPromptChunk,
    FfiLlmProvider, FfiLlmSegmentInput, FfiModelCatalogSelectedIds, FfiModelCatalogSnapshot,
    FfiModelSelectionPaths, FfiOnlineAsrProvider, FfiOnlineAsrProviderRequest,
    FfiPolishSegmentsRequest, FfiPolishedSegment, FfiPresetModel, FfiResolvedModelDownload,
    FfiRuntimePathStatus, FfiSummarizeTranscriptRequest, FfiSummarySegmentInput,
    FfiTranslateSegmentsRequest, FfiTranslatedSegment, FfiVolcengineDoubaoAsrConfig,
    SonaCoreBindingResult, asr_bridge, config_bridge, llm_bridge, model_bridge, runtime_bridge,
};

/// Rust facade used by tests and by the top-level UniFFI exports.
pub struct SonaCoreFacade;

impl SonaCoreFacade {
    pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
        runtime_bridge::normalize_export_format(value)
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

    pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
        runtime_bridge::runtime_path_status(path)
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
