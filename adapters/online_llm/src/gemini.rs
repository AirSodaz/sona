use sona_core::llm::provider_protocol::{
    GeminiGenerateContentRequestParts as CoreGeminiGenerateContentRequestParts,
    build_gemini_generate_content_request_parts,
};
use sona_core::ports::llm::LlmPortError;

use crate::transport::LlmApiUrl;

#[derive(Clone, Debug)]
pub struct GeminiGenerateContentRequestParts {
    pub url: LlmApiUrl,
    pub headers: Vec<(&'static str, String)>,
}

pub fn build_gemini_generate_content_request_parts_for_reqwest(
    base_url: &str,
    model: &str,
    api_key: &str,
    stream: bool,
) -> Result<GeminiGenerateContentRequestParts, LlmPortError> {
    let CoreGeminiGenerateContentRequestParts { url, headers } =
        build_gemini_generate_content_request_parts(base_url, model, api_key, stream)?;
    let url = LlmApiUrl::parse(&url)?;

    Ok(GeminiGenerateContentRequestParts { url, headers })
}
