use sona_core::ports::llm::LlmPortErrorKind;
use sona_online_llm::{LlmApiUrl, validate_llm_api_host};

#[test]
fn api_host_validation_rejects_remote_http_hosts() {
    let error = validate_llm_api_host("http://api.example.com/v1").unwrap_err();

    assert_eq!(error.kind, LlmPortErrorKind::InvalidRequest);
    assert_eq!(
        error.message,
        "LLM API host must use https:// unless it points to localhost."
    );
}

#[test]
fn api_host_validation_accepts_https_and_loopback_http_hosts() {
    for base_url in [
        "https://api.example.com/v1",
        "http://localhost:1234/v1",
        "http://127.0.0.1:11434",
        "http://[::1]:11434",
    ] {
        validate_llm_api_host(base_url)
            .unwrap_or_else(|error| panic!("{base_url} should be accepted: {error}"));
    }
}

#[test]
fn llm_api_url_preserves_policy_after_join_and_query() {
    let root = LlmApiUrl::parse("https://api.example.com/v1").unwrap();

    assert_eq!(
        root.join("/v1/chat/completions").unwrap().as_str(),
        "https://api.example.com/v1/chat/completions"
    );
    assert_eq!(
        root.with_query("api-version=2024-10-21").unwrap().as_str(),
        "https://api.example.com/v1?api-version=2024-10-21"
    );

    let error = LlmApiUrl::parse("http://api.example.com/v1").unwrap_err();
    assert_eq!(error.kind, LlmPortErrorKind::InvalidRequest);
    assert_eq!(
        error.message,
        "LLM API host must use https:// unless it points to localhost."
    );
}
