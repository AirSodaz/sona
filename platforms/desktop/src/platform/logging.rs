use serde_json::{Value, json};
use sona_core::runtime::logging::{sanitize_log_text, sanitize_log_value};

/// Keep legacy host fields while promoting validated client diagnostics to JSON fields.
pub fn format_record(message: &str, record: &log::Record<'_>) -> Value {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut entry = json!({
        "schema_version": 1,
        "time": timestamp_ms / 1000,
        "timestamp_ms": timestamp_ms,
        "level": record.level().to_string(),
        "target": record.target(),
        "file": record.file(),
        "line": record.line(),
        "source": "backend",
        "message": sanitize_log_text(message),
    });
    if let Ok(client) = serde_json::from_str::<Value>(message)
        && client["schema_version"] == 1
        && client["source"] == "frontend"
        && client["message"].is_string()
        && client["component"].is_string()
        && client["event"].is_string()
    {
        // Only these fields are client-owned; never trust level/time/target from payloads.
        for key in ["source", "component", "event", "message", "context"] {
            entry[key] = sanitize_log_value(&client[key]);
        }
    }
    entry
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
    }
}
