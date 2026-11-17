use serde_json::{Value, json};
use sona_core::runtime::logging::{sanitize_log_text, sanitize_log_value};

/// Keep legacy host fields while promoting validated client diagnostics to JSON fields.
pub fn format_record(message: &str, record: &log::Record<'_>) -> Value {
    let timestamp_ms = crate::platform::time::unix_timestamp_millis();
    let mut entry = json!({
        "schema_version": 1,
        "time": timestamp_ms / 1000,
        "timestamp_ms": timestamp_ms,
        "level": record.level().to_string(),
        "target": record.target(),
        "file": record.file(),
        "line": record.line(),
        "source": "backend",
    });
    let trimmed = message.trim_start();
    let client_payload = if trimmed.starts_with('{') && trimmed.contains("\"source\"") {
        serde_json::from_str::<Value>(trimmed).ok()
    } else {
        None
    };
    match client_payload {
        // Only these fields are client-owned; never trust level/time/target from payloads.
        Some(client) if is_client_diagnostic(&client) => {
            for key in ["source", "component", "event", "message", "context"] {
                entry[key] = sanitize_log_value(&client[key]);
            }
        }
        // Client-shaped but malformed: key-based masking still applies, never raw text.
        Some(client) if client["source"] == "frontend" => {
            entry["message"] = json!("[client payload]");
            entry["client"] = sanitize_log_value(&client);
        }
        _ => {
            entry["message"] = json!(sanitize_log_text(message));
        }
    }
    entry
}

fn is_client_diagnostic(client: &Value) -> bool {
    client["source"] == "frontend"
        && client["schema_version"] == 1
        && client["message"].is_string()
        && client["component"].is_string()
        && client["event"].is_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotes_client_context_without_overriding_host_metadata() {
        let record = log::Record::builder()
            .level(log::Level::Warn)
            .target("webview")
            .build();
        let entry = format_record(
            r#"{"schema_version":1,"source":"frontend","component":"BatchQueue","event":"task_start","message":"start","level":"TRACE","context":{"taskId":"task","apiKey":"secret"}}"#,
            &record,
        );
        assert_eq!(entry["level"], "WARN");
        assert_eq!(entry["target"], "webview");
        assert_eq!(entry["context"]["taskId"], "task");
        assert_eq!(entry["context"]["apiKey"], "[redacted]");
    }

    #[test]
    fn sanitizes_plain_backend_and_invalid_client_records() {
        let record = log::Record::builder().build();
        let entry = format_record("failed https://host/?api_key=secret", &record);
        assert_eq!(entry["source"], "backend");
        assert_eq!(entry["message"], "failed [redacted-url]");
        let entry = format_record(r#"{"source":"frontend"}"#, &record);
        assert_eq!(entry["source"], "backend");
        // Client-shaped but malformed payloads keep key-based masking.
        let entry = format_record(
            r#"{"source":"frontend","context":{"text":"private-transcript","endpoint":"wss://host/?token=secret"}}"#,
            &record,
        );
        assert_eq!(entry["source"], "backend");
        assert_eq!(entry["message"], "[client payload]");
        assert_eq!(entry["client"]["context"]["text"], "[redacted]");
        assert_eq!(entry["client"]["context"]["endpoint"], "[redacted-url]");
        assert!(!entry.to_string().contains("private-transcript"));
    }
}
