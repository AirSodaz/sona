use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use serde_json::json;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortErrorKind, LlmTranslationPort, LlmTranslationRequest};
use sona_online_llm::OnlineLlmAdapter;

fn spawn_mock_server(response_body: String) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let mut received = Vec::new();
        loop {
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            received.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&received);
            if let Some(header_end) = text.find("\r\n\r\n") {
                let headers = &text[..header_end];
                let content_len = headers
                    .lines()
                    .find_map(|line| {
                        let (k, v) = line.split_once(':')?;
                        if k.eq_ignore_ascii_case("content-length") {
                            v.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if received.len() >= header_end + 4 + content_len {
                    break;
                }
            }
        }
        let http_resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(http_resp.as_bytes()).unwrap();
        String::from_utf8_lossy(&received).to_string()
    });
    (format!("http://{addr}"), handle)
}

fn base_config(provider: BuiltinLlmProvider, base_url: String) -> LlmConfig {
    let strategy = LlmProviderStrategy::from_provider(&LlmProvider::Builtin(provider.clone()));
    LlmConfig {
        provider: LlmProvider::Builtin(provider),
        strategy,
        base_url,
        api_key: String::new(),
        model: "default".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn deepl_sends_auth_header_and_batch_texts() {
    let body = json!({
        "translations": [
            { "text": "你好" },
            { "text": "世界" }
        ]
    })
    .to_string();
    let (base_url, handle) = spawn_mock_server(body);
    let mut cfg = base_config(BuiltinLlmProvider::Deepl, base_url);
    cfg.api_key = "test-deepl-key:fx".to_string();

    let res = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: cfg,
            texts: vec!["Hello".into(), "World".into()],
            target_language: "zh-CN".into(),
            source_language: Some("en".into()),
        })
        .await
        .unwrap();

    assert_eq!(res, vec!["你好", "世界"]);
    let raw_req = handle.join().unwrap();
    assert!(raw_req.contains("POST /v2/translate HTTP/1.1"));
    assert!(raw_req.to_ascii_lowercase().contains("authorization: deepl-auth-key test-deepl-key:fx"));
    assert!(raw_req.contains("\"target_lang\":\"ZH-HANS\""));
    assert!(raw_req.contains("\"source_lang\":\"EN\""));
}

#[tokio::test]
async fn microsoft_translator_sends_key_region_and_query() {
    let body = json!([
        { "translations": [{ "text": "你好" }] },
        { "translations": [{ "text": "谢谢" }] }
    ])
    .to_string();
    let (base_url, handle) = spawn_mock_server(body);
    let mut cfg = base_config(BuiltinLlmProvider::MicrosoftTranslator, base_url);
    cfg.api_key = "ms-key".to_string();
    cfg.api_region = Some("eastasia".to_string());

    let res = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: cfg,
            texts: vec!["Hello".into(), "Thanks".into()],
            target_language: "zh-CN".into(),
            source_language: None,
        })
        .await
        .unwrap();

    assert_eq!(res, vec!["你好", "谢谢"]);
    let raw_req = handle.join().unwrap();
    assert!(raw_req.contains("/translate?api-version=3.0&to=zh-Hans"));
    assert!(raw_req.to_ascii_lowercase().contains("ocp-apim-subscription-key: ms-key"));
    assert!(raw_req.to_ascii_lowercase().contains("ocp-apim-subscription-region: eastasia"));
}

#[tokio::test]
async fn libre_translate_works_without_api_key() {
    let body = json!({ "translatedText": ["你好", "再见"] }).to_string();
    let (base_url, handle) = spawn_mock_server(body);
    let cfg = base_config(BuiltinLlmProvider::LibreTranslate, base_url);

    let res = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: cfg,
            texts: vec!["Hello".into(), "Bye".into()],
            target_language: "zh-CN".into(),
            source_language: None,
        })
        .await
        .unwrap();

    assert_eq!(res, vec!["你好", "再见"]);
    let raw_req = handle.join().unwrap();
    assert!(raw_req.contains("POST /translate HTTP/1.1"));
    assert!(!raw_req.contains("api_key"));
}

#[tokio::test]
async fn deeplx_appends_translate_path_and_optional_bearer_token() {
    let body = json!({ "code": 200, "data": "你好" }).to_string();
    let (base_url, handle) = spawn_mock_server(body);
    let mut cfg = base_config(BuiltinLlmProvider::Deeplx, base_url);
    cfg.api_key = "dlx-token".to_string();

    let res = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: cfg,
            texts: vec!["Hello".into()],
            target_language: "zh-CN".into(),
            source_language: None,
        })
        .await
        .unwrap();

    assert_eq!(res, vec!["你好"]);
    let raw_req = handle.join().unwrap();
    assert!(raw_req.contains("POST /translate HTTP/1.1"));
    assert!(raw_req.to_ascii_lowercase().contains("authorization: bearer dlx-token"));
}

#[tokio::test]
async fn baidu_translate_signs_form_request_and_maps_errors() {
    let body = json!({
        "trans_result": [{ "src": "apple", "dst": "苹果" }]
    })
    .to_string();
    let (base_url, handle) = spawn_mock_server(body);
    let mut cfg = base_config(BuiltinLlmProvider::BaiduTranslate, base_url);
    cfg.app_id = Some("2015063000000001".to_string());
    cfg.api_key = "12345678".to_string();

    let res = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: cfg,
            texts: vec!["apple".into()],
            target_language: "zh-CN".into(),
            source_language: None,
        })
        .await
        .unwrap();

    assert_eq!(res, vec!["苹果"]);
    let raw_req = handle.join().unwrap();
    assert!(raw_req.contains("appid=2015063000000001"));
    assert!(raw_req.contains("to=zh"));
    assert!(raw_req.contains("sign="));

    let err_body = json!({ "error_code": "54001", "error_msg": "Invalid Sign" }).to_string();
    let (err_url, _) = spawn_mock_server(err_body);
    let mut err_cfg = base_config(BuiltinLlmProvider::BaiduTranslate, err_url);
    err_cfg.app_id = Some("bad".to_string());
    err_cfg.api_key = "bad".to_string();
    let err = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: err_cfg,
            texts: vec!["apple".into()],
            target_language: "zh-CN".into(),
            source_language: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.kind, LlmPortErrorKind::Authentication);
}

#[tokio::test]
async fn youdao_translate_signs_v3_request() {
    let body = json!({
        "errorCode": "0",
        "translation": ["你好"]
    })
    .to_string();
    let (base_url, handle) = spawn_mock_server(body);
    let mut cfg = base_config(BuiltinLlmProvider::YoudaoTranslate, base_url);
    cfg.app_id = Some("yd-app-key".to_string());
    cfg.api_key = "yd-secret".to_string();

    let res = OnlineLlmAdapter
        .translate_batch(LlmTranslationRequest {
            config: cfg,
            texts: vec!["hello".into()],
            target_language: "zh-CN".into(),
            source_language: Some("en".into()),
        })
        .await
        .unwrap();

    assert_eq!(res, vec!["你好"]);
    let raw_req = handle.join().unwrap();
    assert!(raw_req.contains("appKey=yd-app-key"));
    assert!(raw_req.contains("signType=v3"));
    assert!(raw_req.contains("to=zh-CHS"));
}
