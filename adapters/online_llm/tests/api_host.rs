use sona_core::ports::llm::LlmPortErrorKind;
use sona_online_llm::{LlmApiUrl, validate_llm_api_host};

#[test]
fn api_host_validation_rejects_remote_http_hosts() {
    for base_url in [
        "http://api.example.com/v1",
        "http://8.8.8.8:8080",
        "http://1.1.1.1/v1",
        "http://[2001:db8::1]:11434",
    ] {
        let error = validate_llm_api_host(base_url).unwrap_err();
        assert_eq!(error.kind, LlmPortErrorKind::InvalidRequest);
        assert_eq!(
            error.message,
            "LLM API host must use https:// unless it points to a local or LAN address."
        );
    }
}

#[test]
fn api_host_validation_accepts_https_and_local_or_lan_http_hosts() {
    for base_url in [
        // Public HTTPS
        "https://api.example.com/v1",
        // Loopback
        "http://localhost:1234/v1",
        "http://127.0.0.1:11434",
        "http://[::1]:11434",
        "http://0.0.0.0:11434",
        // Private IPv4 (RFC 1918)
        "http://10.0.0.5:8000",
        "http://172.16.0.1:1234",
        "http://172.31.255.255:1234",
        "http://192.168.1.100:11434",
        // Link-local IPv4 (RFC 3927)
        "http://169.254.1.1:8000",
        // Shared address space / CGNAT (RFC 6598, e.g. Tailscale)
        "http://100.64.1.2:11434",
        "http://100.127.255.254:11434",
        // Private / Link-local IPv6 (RFC 4193 & RFC 4291)
        "http://[fc00::1]:11434",
        "http://[fd12:3456:789a::1]:11434",
        "http://[fe80::1]:11434",
        "http://[::ffff:192.168.1.5]:11434",
        // Local and LAN hostnames
        "http://ollama.local:11434",
        "http://ollama:11434",
        "http://my-gpu-server:1234/v1",
        "http://host.docker.internal:11434",
        "http://server.lan:8080",
        "http://home.home.arpa:8080",
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
        "LLM API host must use https:// unless it points to a local or LAN address."
    );
}

#[test]
fn normalize_openai_base_url_responses_strategy_strips_redundant_responses_suffix() {
    use sona_core::llm::tasks::LlmProviderStrategy;
    use sona_online_llm::aimux_adapter::normalize_openai_base_url;

    // Base url without /v1 and api_path with /v1/responses
    assert_eq!(
        normalize_openai_base_url(
            LlmProviderStrategy::OpenAiResponses,
            "https://newapi.example.com",
            Some("/v1/responses")
        ),
        "https://newapi.example.com/v1"
    );

    // Base url with /v1 and api_path with /v1/responses
    assert_eq!(
        normalize_openai_base_url(
            LlmProviderStrategy::OpenAiResponses,
            "https://newapi.example.com/v1",
            Some("/v1/responses")
        ),
        "https://newapi.example.com/v1"
    );

    // Base url with /responses suffix
    assert_eq!(
        normalize_openai_base_url(
            LlmProviderStrategy::OpenAiResponses,
            "https://newapi.example.com/v1/responses",
            None
        ),
        "https://newapi.example.com/v1"
    );

    // Base url plain with /responses path
    assert_eq!(
        normalize_openai_base_url(
            LlmProviderStrategy::OpenAiResponses,
            "https://newapi.example.com/v1",
            Some("/responses")
        ),
        "https://newapi.example.com/v1"
    );
}
