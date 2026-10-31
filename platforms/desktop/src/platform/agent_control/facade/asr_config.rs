use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrTranscriptionRequest, OnlineAsrProviderRequest,
};

pub fn resolve_batch_asr_request_from_config(
    config: &serde_json::Value,
    language_override: Option<&str>,
) -> Result<AsrTranscriptionRequest, String> {
    let batch_selection = config
        .pointer("/asr/selections/batch")
        .or_else(|| config.pointer("/asr/batch"));

    if let Some(batch) = batch_selection {
        let engine = batch
            .get("engine")
            .and_then(|e| e.as_str())
            .unwrap_or("local");
        if engine == "online" {
            let provider_id = batch
                .get("providerId")
                .or_else(|| batch.get("provider_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("volcengine-doubao");

            let profile_id = batch
                .get("profileId")
                .or_else(|| batch.get("profile_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("default");

            let provider_config = config
                .pointer(&format!("/asr/providers/online/{provider_id}"))
                .or_else(|| config.pointer(&format!("/asr/providers/{provider_id}")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            let language = language_override
                .map(ToString::to_string)
                .or_else(|| {
                    batch
                        .get("language")
                        .or_else(|| config.get("batchLanguage"))
                        .or_else(|| config.get("batch_language"))
                        .or_else(|| config.get("language"))
                        .and_then(|l| l.as_str())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "auto".to_string());

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            return Ok(AsrTranscriptionRequest {
                mode: AsrMode::Batch,
                language,
                enable_itn,
                normalization_options: Default::default(),
                postprocess_options: Default::default(),
                hotwords: None,
                speaker_processing: None,
                engine_config: AsrEngineConfig::Online {
                    provider: OnlineAsrProviderRequest {
                        provider_id: provider_id.to_string(),
                        profile_id: profile_id.to_string(),
                        config: provider_config,
                    },
                },
            });
        }

        let model_path = batch
            .get("modelPath")
            .or_else(|| batch.get("model_path"))
            .and_then(|p| p.as_str())
            .unwrap_or("");

        if !model_path.trim().is_empty() {
            let language = language_override
                .map(ToString::to_string)
                .or_else(|| {
                    batch
                        .get("language")
                        .or_else(|| config.get("batchLanguage"))
                        .or_else(|| config.get("batch_language"))
                        .or_else(|| config.get("language"))
                        .and_then(|l| l.as_str())
                        .map(ToString::to_string)
                })
                .unwrap_or_else(|| "auto".to_string());

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let vad_model = batch
                .get("vadModel")
                .or_else(|| batch.get("vad_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let punctuation_model = batch
                .get("punctuationModel")
                .or_else(|| batch.get("punctuation_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let num_threads = batch
                .get("numThreads")
                .or_else(|| batch.get("num_threads"))
                .and_then(|t| t.as_i64())
                .unwrap_or(4) as i32;

            return Ok(AsrTranscriptionRequest::local_sherpa(
                AsrMode::Batch,
                model_path.to_string(),
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                0.5,
                String::new(),
                None,
                None,
                Default::default(),
                Default::default(),
                None,
                None,
            ));
        }
    }

    let legacy_model_path = config
        .get("batchModelPath")
        .or_else(|| config.get("batch_model_path"))
        .and_then(|p| p.as_str())
        .unwrap_or("");

    if !legacy_model_path.trim().is_empty() {
        let language = language_override
            .map(ToString::to_string)
            .or_else(|| {
                config
                    .get("batchLanguage")
                    .or_else(|| config.get("batch_language"))
                    .or_else(|| config.get("language"))
                    .and_then(|l| l.as_str())
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "auto".to_string());

        let enable_itn = config
            .get("enableITN")
            .or_else(|| config.get("enable_itn"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let vad_model = config
            .get("vadModel")
            .or_else(|| config.get("vad_model"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let punctuation_model = config
            .get("punctuationModel")
            .or_else(|| config.get("punctuation_model"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        return Ok(AsrTranscriptionRequest::local_sherpa(
            AsrMode::Batch,
            legacy_model_path.to_string(),
            4,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            0.5,
            String::new(),
            None,
            None,
            Default::default(),
            Default::default(),
            None,
            None,
        ));
    }

    match resolve_live_asr_request_from_config(config) {
        Ok(mut req) => {
            req.mode = AsrMode::Batch;
            if let Some(lang) = language_override {
                req.language = lang.to_string();
            } else if let Some(batch_lang) = config
                .get("batchLanguage")
                .or_else(|| config.get("batch_language"))
                .and_then(|l| l.as_str())
            {
                req.language = batch_lang.to_string();
            }
            Ok(req)
        }
        Err(_) => Err(
            "No batch or live ASR model or provider is configured in Sona client settings"
                .to_string(),
        ),
    }
}

pub fn resolve_live_asr_request_from_config(
    config: &serde_json::Value,
) -> Result<AsrTranscriptionRequest, String> {
    let live_selection = config
        .pointer("/asr/selections/live")
        .or_else(|| config.pointer("/asr/live"));

    if let Some(live) = live_selection {
        let engine = live
            .get("engine")
            .and_then(|e| e.as_str())
            .unwrap_or("local");
        if engine == "online" {
            let provider_id = live
                .get("providerId")
                .or_else(|| live.get("provider_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("volcengine-doubao");

            let profile_id = live
                .get("profileId")
                .or_else(|| live.get("profile_id"))
                .and_then(|p| p.as_str())
                .unwrap_or("default");

            let provider_config = config
                .pointer(&format!("/asr/providers/online/{provider_id}"))
                .or_else(|| config.pointer(&format!("/asr/providers/{provider_id}")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            let language = live
                .get("language")
                .or_else(|| config.get("language"))
                .and_then(|l| l.as_str())
                .unwrap_or("auto")
                .to_string();

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            return Ok(AsrTranscriptionRequest {
                mode: AsrMode::Streaming,
                language,
                enable_itn,
                normalization_options: Default::default(),
                postprocess_options: Default::default(),
                hotwords: None,
                speaker_processing: None,
                engine_config: AsrEngineConfig::Online {
                    provider: OnlineAsrProviderRequest {
                        provider_id: provider_id.to_string(),
                        profile_id: profile_id.to_string(),
                        config: provider_config,
                    },
                },
            });
        }

        let model_path = live
            .get("modelPath")
            .or_else(|| live.get("model_path"))
            .and_then(|p| p.as_str())
            .unwrap_or("");

        if !model_path.trim().is_empty() {
            let language = live
                .get("language")
                .or_else(|| config.get("language"))
                .and_then(|l| l.as_str())
                .unwrap_or("auto")
                .to_string();

            let enable_itn = config
                .get("enableITN")
                .or_else(|| config.get("enable_itn"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let vad_model = live
                .get("vadModel")
                .or_else(|| live.get("vad_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let punctuation_model = live
                .get("punctuationModel")
                .or_else(|| live.get("punctuation_model"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let num_threads = live
                .get("numThreads")
                .or_else(|| live.get("num_threads"))
                .and_then(|t| t.as_i64())
                .unwrap_or(4) as i32;

            return Ok(AsrTranscriptionRequest::local_sherpa(
                AsrMode::Streaming,
                model_path.to_string(),
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                0.5,
                String::new(),
                None,
                None,
                Default::default(),
                Default::default(),
                None,
                None,
            ));
        }
    }

    let legacy_model_path = config
        .get("streamingModelPath")
        .or_else(|| config.get("streaming_model_path"))
        .or_else(|| config.get("modelPath"))
        .or_else(|| config.get("model_path"))
        .and_then(|p| p.as_str())
        .unwrap_or("");

    if !legacy_model_path.trim().is_empty() {
        let language = config
            .get("language")
            .and_then(|l| l.as_str())
            .unwrap_or("auto")
            .to_string();

        let enable_itn = config
            .get("enableITN")
            .or_else(|| config.get("enable_itn"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let vad_model = config
            .get("liveVadModelPath")
            .or_else(|| config.get("live_vad_model_path"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let punctuation_model = config
            .get("livePunctuationModelPath")
            .or_else(|| config.get("live_punctuation_model_path"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        return Ok(AsrTranscriptionRequest::local_sherpa(
            AsrMode::Streaming,
            legacy_model_path.to_string(),
            4,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            0.5,
            String::new(),
            None,
            None,
            Default::default(),
            Default::default(),
            None,
            None,
        ));
    }

    Err("No live ASR model or provider is configured in Sona client settings".to_string())
}
