use aimux_core::error::{AiMuxError, ApiCallError};
use aimux_core::transcription_model::{
    AudioInput, TranscriptionCallOptions, TranscriptionModel, TranscriptionResult,
    TranscriptionSegment,
};
use async_trait::async_trait;
use base64::Engine;
use serde_json::Value;

use crate::{
    VolcengineDoubaoConfigFields, map_volcengine_status_error, segments_from_volcengine_response,
};
pub struct VolcengineTranscriptionModel {
    pub config: VolcengineDoubaoConfigFields,
    pub client: reqwest::Client,
}

impl VolcengineTranscriptionModel {
    pub fn new(config: VolcengineDoubaoConfigFields) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_client(config: VolcengineDoubaoConfigFields, client: reqwest::Client) -> Self {
        Self { config, client }
    }
}

#[async_trait]
impl TranscriptionModel for VolcengineTranscriptionModel {
    fn provider(&self) -> &str {
        "volcengine"
    }

    fn model_id(&self) -> &str {
        &self.config.batch_resource_id
    }

    async fn do_generate(
        &self,
        options: &TranscriptionCallOptions,
    ) -> Result<TranscriptionResult, AiMuxError> {
        let bytes = match &options.audio {
            AudioInput::Binary(bytes) => bytes.clone(),
            AudioInput::Base64(b64) => {
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                    .map_err(|e| AiMuxError::InvalidArgument(format!("invalid base64: {e}")))?
            }
        };

        let audio_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let request_id = uuid::Uuid::new_v4().to_string();

        let format = match options.media_type.as_str() {
            "audio/mp3" | "mp3" => "mp3",
            "audio/ogg" | "ogg" => "ogg",
            "audio/m4a" | "m4a" => "m4a",
            "audio/aac" | "aac" => "aac",
            "audio/flac" | "flac" => "flac",
            _ => "wav",
        };

        let mut body = serde_json::json!({
            "user": {
                "uid": "sona"
            },
            "audio": {
                "format": format,
                "data": audio_data
            },
            "request": {
                "model_name": "bigmodel",
                "enable_itn": true,
                "enable_punc": true,
                "show_utterances": true,
                "result_type": "full"
            }
        });

        // Pass language / hotwords from provider_options if present
        if let Some(volc) = options
            .provider_options
            .as_ref()
            .and_then(|po| po.get("volcengine"))
            && let Some(req_obj) = body.get_mut("request").and_then(Value::as_object_mut)
        {
            if let Some(itn) = volc.get("enable_itn").and_then(Value::as_bool) {
                req_obj.insert("enable_itn".to_string(), serde_json::Value::Bool(itn));
            }
            if let Some(lang) = volc.get("language").and_then(Value::as_str) {
                req_obj.insert(
                    "language".to_string(),
                    serde_json::Value::String(lang.to_string()),
                );
            }
            if let Some(hotwords) = volc.get("hotwords").and_then(Value::as_str) {
                req_obj.insert(
                    "hotwords".to_string(),
                    serde_json::Value::String(hotwords.to_string()),
                );
            }
        }

        let response = self
            .client
            .post(&self.config.batch_endpoint)
            .header("X-Api-Key", &self.config.api_key)
            .header("X-Api-Resource-Id", &self.config.batch_resource_id)
            .header("X-Api-Request-Id", request_id)
            .header("X-Api-Sequence", "-1")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AiMuxError::ApiCall(ApiCallError {
                    status_code: e.status().map(|s| s.as_u16()),
                    provider_code: None,
                    message: format!("Volcengine batch network request failed: {e}"),
                    response_body: None,
                    request_id: None,
                    retry_after_ms: None,
                    is_retryable: e.is_connect() || e.is_timeout(),
                })
            })?;

        let status = response.status();
        let headers = response.headers().clone();
        let api_code = headers
            .get("X-Api-Status-Code")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let api_message = headers
            .get("X-Api-Message")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        if !status.is_success() || api_code.as_deref().is_some_and(|code| code != "20000000") {
            let effective_status = if status.is_success() {
                let code_str = api_code.as_deref().unwrap_or("");
                if code_str == "55000031" {
                    429
                } else if code_str.starts_with("45") || code_str.starts_with("40") {
                    400
                } else if code_str.starts_with('5') {
                    500
                } else {
                    status.as_u16()
                }
            } else {
                status.as_u16()
            };

            let message = map_volcengine_status_error(
                status.as_u16(),
                api_code.as_deref(),
                api_message.as_deref(),
            );
            return Err(AiMuxError::ApiCall(ApiCallError {
                status_code: Some(effective_status),
                provider_code: api_code,
                message,
                response_body: None,
                request_id: None,
                retry_after_ms: None,
                is_retryable: effective_status == 429 || (500..=599).contains(&effective_status),
            }));
        }

        let response_value = response.json::<Value>().await.map_err(|e| {
            AiMuxError::InvalidResponseData(format!(
                "Volcengine batch response parsing failed: {e}"
            ))
        })?;

        let segments_raw = segments_from_volcengine_response(&response_value, true, "volc-batch")
            .map_err(|e| AiMuxError::InvalidResponseData(e.to_string()))?;

        let duration_in_seconds = response_value
            .get("audio_info")
            .and_then(|value| value.get("duration"))
            .and_then(Value::as_f64)
            .map(|ms| ms / 1000.0);
        let text = segments_raw
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let segments = segments_raw
            .into_iter()
            .map(|seg| TranscriptionSegment {
                text: seg.text,
                start_second: seg.start,
                end_second: seg.end,
            })
            .collect();

        Ok(TranscriptionResult {
            text,
            segments,
            language: None,
            duration_in_seconds,
            warnings: Vec::new(),
            request: None,
            response: Default::default(),
            provider_metadata: None,
        })
    }
}
