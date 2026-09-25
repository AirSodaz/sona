use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::time::Instant;

/// Tests the connectivity and authentication of an online ASR provider.
/// Returns elapsed roundtrip latency in milliseconds on success, or a structured port error.
pub async fn test_online_asr_provider(
    provider_id: &str,
    config: &serde_json::Value,
) -> Result<u64, AsrPortError> {
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AsrPortError::runtime(format!("无法创建 HTTP 客户端: {e}")))?;
    let api_key = config
        .get("apiKey")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");

    if api_key.is_empty() {
        return Err(AsrPortError::invalid_request("API Key 不能为空"));
    }

    match provider_id {
        "openai-whisper" => {
            let endpoint = config
                .get("batchEndpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("https://api.openai.com/v1/audio/transcriptions");
            let base_url = endpoint
                .trim_end_matches("/audio/transcriptions")
                .trim_end_matches('/');
            let test_url = format!("{base_url}/models");
            let res = client
                .get(&test_url)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let status = res.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "API Key 无效 (HTTP 401)",
                    ),
                    403 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "权限不足或已封禁 (HTTP 403)",
                    ),
                    code => AsrPortError::runtime(format!("服务返回异常 (HTTP {code})")),
                });
            }
        }
        "groq-whisper" => {
            let endpoint = config
                .get("batchEndpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("https://api.groq.com/openai/v1/audio/transcriptions");
            let base_url = endpoint
                .trim_end_matches("/audio/transcriptions")
                .trim_end_matches('/');
            let test_url = format!("{base_url}/models");
            let res = client
                .get(&test_url)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let status = res.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "API Key 无效 (HTTP 401)",
                    ),
                    403 => {
                        AsrPortError::new(AsrPortErrorKind::InvalidRequest, "权限不足 (HTTP 403)")
                    }
                    code => AsrPortError::runtime(format!("服务返回异常 (HTTP {code})")),
                });
            }
        }
        "mistral-voxtral" => {
            let endpoint = config
                .get("batchEndpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("https://api.mistral.ai/v1/audio/transcriptions");
            let base_url = endpoint
                .trim_end_matches("/audio/transcriptions")
                .trim_end_matches('/');
            let test_url = format!("{base_url}/models");
            let res = client
                .get(&test_url)
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let status = res.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "API Key 无效 (HTTP 401)",
                    ),
                    code => AsrPortError::runtime(format!("服务返回异常 (HTTP {code})")),
                });
            }
        }
        "deepgram" => {
            let test_url = "https://api.deepgram.com/v1/projects";
            let res = client
                .get(test_url)
                .header("Authorization", format!("Token {api_key}"))
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let status = res.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "API Key 无效 (HTTP 401)",
                    ),
                    code => AsrPortError::runtime(format!("服务返回异常 (HTTP {code})")),
                });
            }
        }
        "assemblyai" => {
            let test_url = "https://api.assemblyai.com/v2/account";
            let res = client
                .get(test_url)
                .header("Authorization", api_key)
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let status = res.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "API Key 无效 (HTTP 401)",
                    ),
                    code => AsrPortError::runtime(format!("服务返回异常 (HTTP {code})")),
                });
            }
        }
        "elevenlabs" => {
            let test_url = "https://api.elevenlabs.io/v1/user";
            let res = client
                .get(test_url)
                .header("xi-api-key", api_key)
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let status = res.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "API Key 无效 (HTTP 401)",
                    ),
                    code => AsrPortError::runtime(format!("服务返回异常 (HTTP {code})")),
                });
            }
        }
        "volcengine-doubao" => {
            let endpoint = config
                .get("batchEndpoint")
                .and_then(|v| v.as_str())
                .unwrap_or("https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash");
            let resource_id = config
                .get("batchResourceId")
                .and_then(|v| v.as_str())
                .unwrap_or("volc.bigasr.auc_turbo");

            // Standard minimal silent WAV PCM 16kHz 1ch header (44 bytes)
            let dummy_wav_b64 = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAwF0AAIC7AAACABAAZGF0YQAAAAA=";
            let body = serde_json::json!({
                "user": { "uid": "sona-test" },
                "audio": { "format": "wav", "data": dummy_wav_b64 },
                "request": {
                    "model_name": "bigmodel",
                    "result_type": "full"
                }
            });

            let res = client
                .post(endpoint)
                .header("X-Api-Key", api_key)
                .header("X-Api-Resource-Id", resource_id)
                .header("X-Api-Request-Id", uuid::Uuid::new_v4().to_string())
                .header("X-Api-Sequence", "-1")
                .json(&body)
                .send()
                .await
                .map_err(|e| AsrPortError::runtime(format!("网络连接失败: {e}")))?;

            let headers = res.headers();
            let status = res.status();
            let api_code = headers
                .get("X-Api-Status-Code")
                .and_then(|v| v.to_str().ok());
            let api_msg = headers.get("X-Api-Message").and_then(|v| v.to_str().ok());

            if status.as_u16() == 401 || status.as_u16() == 403 || api_code == Some("45000001") {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    "API Key 或 Resource ID 无效",
                ));
            }

            if !status.is_success() && api_code.is_none() {
                return Err(AsrPortError::runtime(format!(
                    "服务返回异常 (HTTP {})",
                    status.as_u16()
                )));
            }

            if let Some(code) = api_code.filter(|c| c.starts_with("45") || c.starts_with("40")) {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    api_msg.unwrap_or(code),
                ));
            }
        }
        _ => {
            return Err(AsrPortError::invalid_request(format!(
                "不支持的服务商: {provider_id}"
            )));
        }
    }

    Ok(start.elapsed().as_millis() as u64)
}
