use serde_json::json;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};

#[test]
fn serializes_asr_port_errors_as_stable_code_and_message() {
    let error = AsrPortError::new(
        AsrPortErrorKind::Unsupported,
        "不支持的在线 ASR provider：future-provider",
    )
    .with_code("UNSUPPORTED_ONLINE_PROVIDER");

    let value = serde_json::to_value(&error).unwrap();

    assert_eq!(value["code"], "UNSUPPORTED_ONLINE_PROVIDER");
    assert!(
        value["message"]
            .as_str()
            .expect("error message should be a string")
            .contains("future-provider")
    );
}

#[test]
fn asr_port_error_code_falls_back_to_kind_when_no_override() {
    let error = AsrPortError::new(AsrPortErrorKind::Network, "connection failed");
    let value = serde_json::to_value(&error).unwrap();
    assert_eq!(value["code"], "NETWORK");
    assert_eq!(value["message"], "connection failed");
}

#[test]
fn asr_port_error_serializes_with_override_code() {
    let value = serde_json::to_value(
        AsrPortError::new(AsrPortErrorKind::Unsupported, "provider foo 不支持流式识别")
            .with_code("STREAMING_NOT_SUPPORTED"),
    )
    .unwrap();

    assert_eq!(
        value,
        json!({
            "code": "STREAMING_NOT_SUPPORTED",
            "message": "provider foo 不支持流式识别"
        })
    );
}

#[test]
fn all_error_kinds_produce_stable_codes() {
    use AsrPortErrorKind::*;
    let cases = [
        (InvalidRequest, "INVALID_REQUEST"),
        (FileSystem, "FILE_SYSTEM"),
        (Model, "MODEL_ERROR"),
        (Authentication, "AUTHENTICATION"),
        (RateLimited, "RATE_LIMITED"),
        (Timeout, "TIMEOUT"),
        (Network, "NETWORK"),
        (Protocol, "PROTOCOL"),
        (Unsupported, "UNSUPPORTED"),
        (Unavailable, "UNAVAILABLE"),
        (Runtime, "RUNTIME"),
    ];
    for (kind, expected_code) in cases {
        let error = AsrPortError::new(kind, "test");
        assert_eq!(error.code(), expected_code, "kind {kind:?} should map to {expected_code}");
        assert_eq!(
            serde_json::to_value(&error).unwrap()["code"],
            expected_code,
        );
    }
}
