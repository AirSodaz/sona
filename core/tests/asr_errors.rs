use serde_json::json;
use sona_core::ports::asr::SherpaError;

#[test]
fn serializes_asr_runtime_errors_as_stable_code_and_message() {
    let value = serde_json::to_value(SherpaError::UnsupportedOnlineProvider {
        provider_id: "future-provider".to_string(),
    })
    .unwrap();

    assert_eq!(value["code"], "UNSUPPORTED_ONLINE_PROVIDER");
    assert!(
        value["message"]
            .as_str()
            .expect("error message should be a string")
            .contains("future-provider")
    );
}

#[test]
fn string_errors_serialize_as_generic_asr_runtime_errors() {
    let value = serde_json::to_value(SherpaError::from("plain adapter failure")).unwrap();

    assert_eq!(
        value,
        json!({
            "code": "GENERIC_ERROR",
            "message": "plain adapter failure"
        })
    );
}

#[test]
fn streaming_transport_errors_expose_stable_codes() {
    let cases = [
        (
            SherpaError::VolcengineWebSocketReadFailed {
                error: "connection reset".to_string(),
            },
            "VOLCENGINE_WEB_SOCKET_READ_FAILED",
        ),
        (
            SherpaError::VolcengineWebSocketClosed,
            "VOLCENGINE_WEB_SOCKET_CLOSED",
        ),
        (
            SherpaError::VolcengineFinalResponseTimeout,
            "VOLCENGINE_FINAL_RESPONSE_TIMEOUT",
        ),
    ];

    for (error, expected_code) in cases {
        assert_eq!(error.code(), expected_code);
        assert_eq!(serde_json::to_value(error).unwrap()["code"], expected_code);
    }
}
