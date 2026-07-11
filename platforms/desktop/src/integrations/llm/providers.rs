use super::*;
use log::warn;
use tauri::{AppHandle, Emitter};

#[cfg(test)]
pub(crate) use sona_online_llm::build_gemini_generate_content_request_parts_for_reqwest as build_gemini_generate_content_request_parts;
pub(crate) use sona_online_llm::{
    GoogleTranslateResponse, OnlineLlmAdapter as DesktopLlmAdapter, generate_text_with_provider,
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
