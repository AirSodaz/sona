use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

/// Bytes scanned by the redaction regexes. Output is capped well below this, so
/// scanning a larger payload cannot surface more detail but can stall a logger.
const MAX_SCANNED_BYTES: usize = 64 * 1024;
const MAX_LOG_TEXT_CHARS: usize = 4096;

/// Only the schemes this application consumes are guaranteed, but any
/// `scheme://` form is redacted: RTSP/RTMP camera inputs and file URLs carry
/// userinfo, credentials or user profile paths just like HTTP ones.
static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b[a-z][a-z0-9+.-]*://[^\s"'<>]+"#).expect("valid URL regex")
});
static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(api[_-]?key|access[_-]?token|token|authorization|password|secret|cookie|passphrase)(["']?\s*[=:]\s*)((?:[A-Za-z_][A-Za-z0-9_]*\(\s*)?(?:basic|bearer|digest)\s+[^&\s,;"'<>]+|(?:[A-Za-z_][A-Za-z0-9_]*\(\s*)?["']?[^&\s,;"'<>]+["']?)"#,
    )
    .expect("valid secret regex")
});
static BEARER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+").expect("valid bearer regex")
});

/// Object keys whose whole value is opaque: never useful in a diagnostic and
/// routinely secret or transcript-bearing.
const OPAQUE_KEYS: [&str; 9] = [
    "config",
    "headers",
    "text",
    "transcript",
    "audio",
    "payload",
    "body",
    "windowtitle",
    "prompt",
];
/// Key fragments matched as substrings so qualified names (`x-api-key`,
/// `userToken`, `clientSecret`, `db_password`, `accessKeyId`, `privateKey`)
/// cannot slip through.
const SENSITIVE_KEY_FRAGMENTS: [&str; 13] = [
    "apikey",
    "token",
    "authorization",
    "password",
    "passphrase",
    "secret",
    "credential",
    "cookie",
    "signature",
    "accesskey",
    "privatekey",
    "auth",
    "sessionkey",
];

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
    OPAQUE_KEYS.contains(&normalized.as_str())
        || SENSITIVE_KEY_FRAGMENTS
            .iter()
            .any(|fragment| normalized.contains(fragment))
}

fn bounded(input: &str) -> &str {
    if input.len() <= MAX_SCANNED_BYTES {
        return input;
    }
    let mut end = MAX_SCANNED_BYTES;
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

pub fn sanitize_log_text(input: &str) -> String {
    if input.len() <= MAX_LOG_TEXT_CHARS
        && !input.contains("://")
        && !input.contains('=')
        && !input.contains(':')
        && !input
            .as_bytes()
            .windows(6)
            .any(|w| w.eq_ignore_ascii_case(b"bearer"))
        && !input.chars().any(char::is_control)
    {
        return input.to_string();
    }
    let without_urls = URL_RE.replace_all(bounded(input), "[redacted-url]");
    let without_bearers = BEARER_RE.replace_all(&without_urls, "Bearer [redacted]");
    SECRET_RE
        .replace_all(&without_bearers, "$1$2[redacted]")
        .chars()
        .take(MAX_LOG_TEXT_CHARS)
        // A raw control character lets one field forge extra log lines.
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

/// Redacts credentials (keys, tokens, passwords, bearer tokens) and an optional
/// explicit secret without obliterating the entire URL, suitable for error messages.
pub fn sanitize_credentials_text(input: &str, explicit_secret: Option<&str>) -> String {
    let mut text = input.to_string();
    if let Some(secret) = explicit_secret {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            text = text.replace(trimmed, "[REDACTED]");
        }
    }
    let without_bearers = BEARER_RE.replace_all(&text, "Bearer [REDACTED]");
    let sanitized = SECRET_RE.replace_all(&without_bearers, "$1$2[REDACTED]");
    sanitized.chars().take(MAX_LOG_TEXT_CHARS).collect()
}

/// Bounded, redacted correlation label for an observer-supplied diagnostic id.
///
/// `BatchTranscriptionObserver::diagnostic_id` may come from caller input, so it
/// is treated like any other log payload instead of being interpolated raw.
pub fn diagnostic_label(id: Option<&str>) -> String {
    match id.map(str::trim) {
        Some(id) if !id.is_empty() => sanitize_log_text(id),
        _ => "unassigned".to_string(),
    }
}

/// Reports a URL's scheme and authority only, dropping userinfo, path and query.
///
/// Credentials and signed tokens live in the userinfo, path and query, while the
/// host is what makes a failure diagnosable, so endpoints stay legible in logs.
pub fn redacted_endpoint(url: &str) -> String {
    let trimmed = url.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return "[redacted-url]".to_string();
    };
    if scheme.is_empty()
        || !scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
    {
        return "[redacted-url]".to_string();
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host: String = authority
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(120)
        .collect();
    if host.is_empty() {
        return "[redacted-url]".to_string();
    }
    format!("{}://{host}", scheme.to_ascii_lowercase())
}

pub fn sanitize_log_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        if is_sensitive_key(key) {
                            Value::String("[redacted]".to_string())
                        } else {
                            sanitize_log_value(value)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(sanitize_log_value).collect()),
        Value::String(value) => Value::String(sanitize_log_text(value)),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_urls_credentials_and_nested_context() {
        assert_eq!(
            sanitize_log_text("request https://user:pass@example.com/x?api_key=secret token=other"),
            "request [redacted-url] token=[redacted]"
        );
        let value = serde_json::json!({
            "context": {
                "webhookSecret": "secret",
                "endpoint": "wss://host/path?token=secret",
                "count": 2,
                "windowTitle": "Private Banking - Account 123",
                "prompt": "Sensitive system prompt"
            }
        });
        let safe = sanitize_log_value(&value);
        assert_eq!(safe["context"]["webhookSecret"], "[redacted]");
        assert_eq!(safe["context"]["endpoint"], "[redacted-url]");
        assert_eq!(safe["context"]["count"], 2);
        assert_eq!(safe["context"]["windowTitle"], "[redacted]");
        assert_eq!(safe["context"]["prompt"], "[redacted]");
    }

    #[test]
    fn sanitizes_headers_and_bounds_unicode_without_panicking() {
        let safe =
            sanitize_log_text("Authorization: Bearer private-token password=private-password");
        assert!(!safe.contains("private-"));
        assert_eq!(sanitize_log_text(&"文".repeat(5000)).chars().count(), 4096);
        let safe = sanitize_log_value(&serde_json::json!({
            "args": [{"refresh_token": "private-token", "text": "private-text"}],
            "config": {"private": "private-config"}
        }));
        assert!(!safe.to_string().contains("private-"));
    }

    #[test]
    fn redacts_quoted_and_scheme_qualified_credentials() {
        for secret in [
            r#"{"api_key":"private-json"}"#,
            r#"password: "private-quoted""#,
            r#"password: Some("private-debug")"#,
            "Authorization: Basic cHJpdmF0ZS1iYXNpYw==",
            "Cookie: session=private-cookie",
            r#"'token': 'private-single'"#,
        ] {
            assert!(
                !sanitize_log_text(secret).contains("private-"),
                "leaked: {secret}"
            );
        }
    }

    #[test]
    fn redacts_camera_and_file_stream_urls() {
        for url in [
            "rtsp://admin:private-pw@192.168.1.10:554/Streaming/Channels/101",
            "rtmp://private-pw@relay.example/live",
            "file:///C:/Users/private-user/audio.wav",
            "srt://private-host:9000",
        ] {
            assert_eq!(sanitize_log_text(url), "[redacted-url]");
        }
    }

    #[test]
    fn keeps_endpoint_host_while_dropping_userinfo_and_query() {
        assert_eq!(
            redacted_endpoint("rtsp://admin:private@camera.local:554/live?api_key=private"),
            "rtsp://camera.local:554"
        );
        assert_eq!(
            redacted_endpoint("wss://api.deepgram.com/v1/listen?token=private"),
            "wss://api.deepgram.com"
        );
        assert_eq!(
            redacted_endpoint("https://user:p@ss@host.example/x"),
            "https://host.example"
        );
        assert_eq!(redacted_endpoint("not a url"), "[redacted-url]");
        assert_eq!(redacted_endpoint("https://"), "[redacted-url]");
    }

    #[test]
    fn redacts_qualified_sensitive_keys() {
        let safe = sanitize_log_value(&serde_json::json!({
            "requestHeaders": {
                "x-api-key": "private-1",
                "userToken": "private-2",
                "clientSecret": "private-3",
                "db_password": "private-4",
                "cookie": "private-5",
                "signature": "private-6",
                "accessKeyId": "private-7",
                "privateKey": "private-8",
                "auth": "private-9",
                "sessionKey": "private-10"
            },
            "kept": "visible"
        }));
        assert!(!safe.to_string().contains("private-"), "{safe}");
        assert_eq!(safe["kept"], "visible");
    }

    #[test]
    fn strips_control_characters_that_would_forge_log_lines() {
        let forged = "auto\n2026-11-17T00:00:00Z ERROR [ServerJob] task_failed job_id=1\tafter";
        let safe = sanitize_log_text(forged);
        assert!(!safe.contains('\n') && !safe.contains('\t'), "{safe}");
        assert!(safe.starts_with("auto 2026-11-17T00:00:00Z"), "{safe}");
    }

    #[test]
    fn labels_diagnostic_ids_without_leaking_caller_input() {
        assert_eq!(diagnostic_label(None), "unassigned");
        assert_eq!(diagnostic_label(Some("  ")), "unassigned");
        assert_eq!(diagnostic_label(Some("batch-1")), "batch-1");
        assert_eq!(
            diagnostic_label(Some("https://host/?api_key=private")),
            "[redacted-url]"
        );
    }

    #[test]
    fn bounds_oversized_input_before_regex_scanning() {
        let oversized = format!("{} https://host/?api_key=private", "x".repeat(200_000));
        let safe = sanitize_log_text(&oversized);
        assert_eq!(safe.chars().count(), 4096);
        assert!(!safe.contains("private"));
    }

    #[test]
    fn credentials_text_redacts_keys_and_tokens_while_preserving_url_structure() {
        let err = "Failed to connect to http://127.0.0.1:8096/emby/System/Info?api_key=secret_12345&foo=bar";
        let sanitized = sanitize_credentials_text(err, Some("secret_12345"));
        assert!(!sanitized.contains("secret_12345"));
        assert!(sanitized.contains("api_key=[REDACTED]"));
        assert!(sanitized.contains("&foo=bar"));

        let err_no_key_provided = "Error at http://emby.local?Api_Key=supersecret&token=tok_secret";
        let sanitized_no_key = sanitize_credentials_text(err_no_key_provided, None);
        assert!(!sanitized_no_key.contains("supersecret"));
        assert!(!sanitized_no_key.contains("tok_secret"));
        assert!(sanitized_no_key.contains("Api_Key=[REDACTED]"));
        assert!(sanitized_no_key.contains("token=[REDACTED]"));
    }

    #[test]
    fn fast_path_preserves_clean_log_messages() {
        let clean = "Worker pool initialized with 4 threads for speech recognition";
        assert_eq!(sanitize_log_text(clean), clean);
    }
}
