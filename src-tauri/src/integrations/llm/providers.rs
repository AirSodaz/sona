use super::*;
use log::warn;
use tauri::{AppHandle, Emitter};

pub(crate) use sona_online_llm::{
    GoogleTranslateRequest, GoogleTranslateResponse, OnlineLlmAdapter as DesktopLlmAdapter,
    build_gemini_generate_content_request_parts_for_reqwest as build_gemini_generate_content_request_parts,
    extract_text_response, generate_text_with_provider, token_usage_from_rig_usage,
};

pub(crate) fn emit_llm_usage_event(
    app: &AppHandle,
    config: &LlmConfig,
    category: LlmUsageCategory,
    occurred_at: String,
    usage: Option<TokenUsage>,
) {
    let payload = LlmUsageEventPayload {
        occurred_at,
        provider: config.provider.clone(),
        model: config.model.clone(),
        category,
        usage,
    };

    if let Err(error) = app.emit(LLM_USAGE_RECORDED_EVENT, payload) {
        warn!(
            "[LLM] failed to emit usage event: provider={:?} category={:?} error={}",
            config.provider, category, error
        );
    }
}
