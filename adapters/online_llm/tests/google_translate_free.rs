use serde_json::json;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmTranslationPort, LlmTranslationRequest};
use sona_online_llm::{
    GOOGLE_TRANSLATE_USER_AGENT, LlmApiUrl, OnlineLlmAdapter,
    build_google_translate_free_candidate_urls, extract_google_translate_free_translation,
    fetch_google_translate_free_translation,
};

#[test]
fn build_candidate_urls_prioritizes_working_endpoints_and_avoids_gtx_first() {
    let base_url = LlmApiUrl::parse("https://translate.googleapis.com/translate_a/single").unwrap();
    let candidates = build_google_translate_free_candidate_urls(&base_url, "zh-CN", "hello world");

    assert!(!candidates.is_empty());
    // Candidate 1 must use client=at to bypass the 429 block on client=gtx
    let first = candidates[0].as_str();
    assert!(
        first.contains("client=at"),
        "first candidate should be client=at: {first}"
    );
    assert!(
        first.contains("translate.googleapis.com"),
        "first candidate domain mismatch: {first}"
    );

    // Candidate 2 must provide the Chrome extension fallback endpoint
    let second = candidates[1].as_str();
    assert!(
        second.contains("clients5.google.com"),
        "second candidate should use clients5: {second}"
    );
    assert!(
        second.contains("client=dict-chrome-ex"),
        "second candidate should use dict-chrome-ex: {second}"
    );

    // Candidate 3 provides translate.google.com with client=at
    let third = candidates[2].as_str();
    assert!(
        third.contains("translate.google.com"),
        "third candidate should use translate.google.com: {third}"
    );
    assert!(
        third.contains("client=at"),
        "third candidate should use client=at: {third}"
    );

    // Candidate 4 is legacy client=gtx
    let fourth = candidates[3].as_str();
    assert!(
        fourth.contains("client=gtx"),
        "fourth candidate should be legacy client=gtx: {fourth}"
    );
}

#[test]
fn build_candidate_urls_handles_clients5_base_url() {
    let base_url = LlmApiUrl::parse("https://clients5.google.com/translate_a/t").unwrap();
    let candidates = build_google_translate_free_candidate_urls(&base_url, "fr", "test");

    assert!(!candidates.is_empty());
    assert!(candidates[0].as_str().contains("client=dict-chrome-ex"));
}

#[test]
fn extract_google_translate_free_translation_handles_format_a_nested_array() {
    let body = json!([
        [
            ["你好，", "hello, ", null, null, 10],
            ["世界！", "world!", null, null, 10]
        ],
        null,
        "en"
    ]);

    let text = extract_google_translate_free_translation(&body).unwrap();
    assert_eq!(text, "你好，世界！");
}

#[test]
fn extract_google_translate_free_translation_handles_format_b_dict_chrome_ex_sl_auto() {
    let body = json!([["Bonjour le monde", "en"]]);

    let text = extract_google_translate_free_translation(&body).unwrap();
    assert_eq!(text, "Bonjour le monde");
}

#[test]
fn extract_google_translate_free_translation_handles_format_c_flat_array() {
    let body = json!(["Hola mundo"]);

    let text = extract_google_translate_free_translation(&body).unwrap();
    assert_eq!(text, "Hola mundo");
}

#[test]
fn extract_google_translate_free_translation_rejects_empty() {
    let body = json!([]);
    assert!(extract_google_translate_free_translation(&body).is_err());

    let body_empty_strings = json!([[""]]);
    assert!(extract_google_translate_free_translation(&body_empty_strings).is_err());
}

#[test]
fn user_agent_is_modern_browser() {
    assert!(GOOGLE_TRANSLATE_USER_AGENT.contains("Mozilla/5.0"));
    assert!(GOOGLE_TRANSLATE_USER_AGENT.contains("Chrome/"));
}

#[tokio::test]
async fn live_fetch_google_translate_free_succeeds_without_429() {
    let base_url = LlmApiUrl::parse("https://translate.googleapis.com/translate_a/single").unwrap();
    let client = base_url.client(Some(15)).unwrap();

    let result =
        fetch_google_translate_free_translation(&client, &base_url, "zh-CN", "hello").await;
    match result {
        Ok(translated) => {
            assert!(
                translated.contains("你好") || translated.contains("您好"),
                "expected translation of 'hello' to contain 你好, got: {translated}"
            );
        }
        Err(err) => {
            panic!("Live Google Translate request failed: {err:?}");
        }
    }
}

#[tokio::test]
async fn live_adapter_translate_batch_succeeds() {
    let adapter = OnlineLlmAdapter;
    let request = LlmTranslationRequest {
        config: LlmConfig {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::GoogleTranslateFree),
            strategy: LlmProviderStrategy::GoogleTranslateFree,
            base_url: "https://translate.googleapis.com/translate_a/single".to_string(),
            api_key: "".to_string(),
            model: "default".to_string(),
            timeout_seconds: Some(15),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
        },
        texts: vec![
            "Good morning".to_string(),
            "Thank you very much".to_string(),
        ],
        target_language: "zh-CN".to_string(),
    };

    let result = adapter.translate_batch(request).await;
    match result {
        Ok(translations) => {
            assert_eq!(translations.len(), 2);
            assert!(!translations[0].is_empty());
            assert!(!translations[1].is_empty());
        }
        Err(err) => {
            panic!("Live translate_batch failed: {err:?}");
        }
    }
}
