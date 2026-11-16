use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:https?|wss?)://[^\s\"'<>]+"#).expect("valid URL regex")
});
static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(api[_-]?key|access[_-]?token|token|authorization|password|secret)(\s*[=:]\s*)([^&\s,;\"'<>]+)"#)
        .expect("valid secret regex")
});
static BEARER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+").expect("valid bearer regex")
});

pub fn sanitize_log_text(input: &str) -> String {
    let without_urls = URL_RE.replace_all(input, "[redacted-url]");
    let without_bearers = BEARER_RE.replace_all(&without_urls, "Bearer [redacted]");
    SECRET_RE
        .replace_all(&without_bearers, "$1$2[redacted]")
        .chars()
        .take(4096)
        .collect()
}

pub fn sanitize_log_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let sensitive = matches!(
                        key.to_ascii_lowercase().replace(['-', '_'], "").as_str(),
                        "apikey"
                            | "token"
                            | "accesstoken"
                            | "refreshtoken"
                            | "authorization"
                            | "password"
                            | "secret"
                            | "webhooksecret"
                            | "config"
                            | "headers"
                            | "text"
                            | "transcript"
                            | "audio"
                            | "payload"
                    );
                    (
                        key.clone(),
                        if sensitive {
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
                "count": 2
            }
        });
        let safe = sanitize_log_value(&value);
        assert_eq!(safe["context"]["webhookSecret"], "[redacted]");
        assert_eq!(safe["context"]["endpoint"], "[redacted-url]");
        assert_eq!(safe["context"]["count"], 2);
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
}
