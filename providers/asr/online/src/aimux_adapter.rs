use std::path::Path;
use std::sync::Arc;

use aimux_core::transcription_model::{AudioInput, TranscriptionCallOptions, TranscriptionModel};
use aimux_providers::assemblyai::{AssemblyAIConfig, AssemblyAITranscriptionModel};
use aimux_providers::deepgram::{DeepgramConfig, DeepgramTranscriptionModel};
use aimux_providers::elevenlabs::{ElevenLabsConfig, ElevenLabsTranscriptionModel};
use aimux_providers::openai::{OpenAIConfig, OpenAITranscriptionModel};
use serde_json::Value;
use sona_core::ports::asr::{
    ASSEMBLYAI_PROVIDER_ID, AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind,
    AsrTranscriptionRequest, DEEPGRAM_PROVIDER_ID, ELEVENLABS_PROVIDER_ID,
    GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, OPENAI_WHISPER_PROVIDER_ID,
    OnlineBatchTranscriptionOutput, OnlineBatchTranscriptionRequest, VOLCENGINE_DOUBAO_PROVIDER_ID,
    find_online_asr_provider,
};
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
    TranscriptTimingUnit,
};

use crate::error::map_aimux_asr_error;
use crate::volcengine::VolcengineTranscriptionModel;
use crate::{
    VolcengineMode, create_cloud_speaker, resolve_online_asr_provider_id,
    resolve_volcengine_config, segments_from_volcengine_response,
};

/// Normalized fields resolved from an online ASR provider request and manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineProviderConfigFields {
    pub api_key: String,
    pub model: String,
    pub batch_endpoint: Option<String>,
}

/// Helper to resolve standard online provider configuration (API key, model, endpoint).
pub fn resolve_online_provider_config(
    request: &AsrTranscriptionRequest,
    provider_id: &str,
) -> Result<OnlineProviderConfigFields, AsrPortError> {
    let provider_request = match &request.engine_config {
        AsrEngineConfig::Online { provider } => provider,
        _ => {
            return Err(AsrPortError::invalid_request(format!(
                "Online ASR provider request is missing for {provider_id}."
            )));
        }
    };

    let manifest = find_online_asr_provider(provider_id).ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("Provider {provider_id} not found in manifest"),
        )
    })?;

    let defaults = manifest.defaults.as_object();

    let get_string = |key: &str| -> String {
        provider_request
            .config
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .or_else(|| {
                defaults
                    .and_then(|d| d.get(key))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
            })
            .unwrap_or("")
            .to_string()
    };

    let api_key = get_string("apiKey");
    if api_key.is_empty() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Authentication,
            format!("{provider_id} API Key is not configured."),
        ));
    }

    let model = get_string("model");
    let batch_endpoint_str = get_string("batchEndpoint");
    let batch_endpoint = if batch_endpoint_str.is_empty() {
        None
    } else {
        Some(batch_endpoint_str)
    };

    Ok(OnlineProviderConfigFields {
        api_key,
        model,
        batch_endpoint,
    })
}

/// Detect MIME type based on audio file extension.
pub fn detect_audio_mime_type(file_path: &Path) -> &'static str {
    match file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "wav" => "audio/wav",
        "mp3" => "audio/mp3",
        "ogg" => "audio/ogg",
        "m4a" => "audio/m4a",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        _ => "audio/wav",
    }
}

fn normalize_language_hint(language: &str) -> Option<String> {
    let lang = language.trim();
    if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(lang.to_string())
    }
}

/// Create an aimux [`TranscriptionModel`] from a Sona [`AsrTranscriptionRequest`].
pub fn create_aimux_transcription_model(
    request: &AsrTranscriptionRequest,
) -> Result<Arc<dyn TranscriptionModel>, AsrPortError> {
    if request.mode != AsrMode::Batch {
        return Err(AsrPortError::invalid_request(format!(
            "Online provider {} can only be used in batch mode.",
            request.provider_id()
        )));
    }

    let provider_id = resolve_online_asr_provider_id(request)?;

    match provider_id {
        VOLCENGINE_DOUBAO_PROVIDER_ID => {
            let config = resolve_volcengine_config(request, VolcengineMode::Batch)
                .map_err(AsrPortError::from)?;
            Ok(Arc::new(VolcengineTranscriptionModel::new(config)))
        }
        OPENAI_WHISPER_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, OPENAI_WHISPER_PROVIDER_ID)?;
            let mut openai_config = OpenAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                openai_config = openai_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "whisper-1".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(OpenAITranscriptionModel::new(
                model_id,
                openai_config,
            )))
        }
        GROQ_WHISPER_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, GROQ_WHISPER_PROVIDER_ID)?;
            let mut openai_config = OpenAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                openai_config = openai_config.with_base_url(base_url);
            }
            openai_config.provider = "groq".to_string();
            let model_id = if config.model.is_empty() {
                "whisper-large-v3-turbo".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(OpenAITranscriptionModel::new(
                model_id,
                openai_config,
            )))
        }
        MISTRAL_VOXTRAL_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, MISTRAL_VOXTRAL_PROVIDER_ID)?;
            let mut openai_config = OpenAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/audio/transcriptions")
                    .trim_end_matches('/');
                openai_config = openai_config.with_base_url(base_url);
            }
            openai_config.provider = "mistral".to_string();
            let model_id = if config.model.is_empty() {
                "mistral-small-latest".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(OpenAITranscriptionModel::new(
                model_id,
                openai_config,
            )))
        }
        DEEPGRAM_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, DEEPGRAM_PROVIDER_ID)?;
            let mut dg_config = DeepgramConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v1/listen")
                    .trim_end_matches('/');
                dg_config = dg_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "nova-2".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(DeepgramTranscriptionModel::new(
                model_id, dg_config,
            )))
        }
        ASSEMBLYAI_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, ASSEMBLYAI_PROVIDER_ID)?;
            let mut aai_config = AssemblyAIConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v2/transcript")
                    .trim_end_matches('/');
                aai_config = aai_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "best".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(AssemblyAITranscriptionModel::new(
                model_id, aai_config,
            )))
        }
        ELEVENLABS_PROVIDER_ID => {
            let config = resolve_online_provider_config(request, ELEVENLABS_PROVIDER_ID)?;
            let mut el_config = ElevenLabsConfig::new(&config.api_key);
            if let Some(endpoint) = &config.batch_endpoint {
                let base_url = endpoint
                    .trim_end_matches("/v1/speech-to-text")
                    .trim_end_matches('/');
                el_config = el_config.with_base_url(base_url);
            }
            let model_id = if config.model.is_empty() {
                "scribe_v1".to_string()
            } else {
                config.model
            };
            Ok(Arc::new(ElevenLabsTranscriptionModel::new(
                model_id, el_config,
            )))
        }
        _ => Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("不支持的在线 ASR provider：{provider_id}"),
        )
        .with_code("UNSUPPORTED_ONLINE_PROVIDER")),
    }
}

/// Returns whether speaker diarization is enabled for an online ASR request.
/// Defaults to `true` if not explicitly set to `false`.
pub fn is_cloud_speaker_diarization_enabled(request: &AsrTranscriptionRequest) -> bool {
    if let sona_core::ports::asr::AsrEngineConfig::Online { provider } = &request.engine_config {
        if let Some(b) = provider
            .config
            .get("speakerDiarization")
            .and_then(Value::as_bool)
        {
            return b;
        }
        if let Some(b) = provider
            .config
            .get("speaker_diarization")
            .and_then(Value::as_bool)
        {
            return b;
        }
    }
    true
}

/// Execute batch transcription through an aimux [`TranscriptionModel`].
pub async fn execute_aimux_batch(
    model: &dyn TranscriptionModel,
    input: OnlineBatchTranscriptionRequest,
) -> Result<OnlineBatchTranscriptionOutput, AsrPortError> {
    let bytes = tokio::fs::read(&input.file_path).await.map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to read audio file: {error}"),
        )
    })?;

    let media_type = detect_audio_mime_type(&input.file_path);
    let mut call_options =
        TranscriptionCallOptions::new(AudioInput::Binary(bytes.clone()), media_type);

    // Build provider options
    let mut po = std::collections::HashMap::new();

    // 1. OpenAI-compatible options
    let mut openai_options = serde_json::Map::new();
    if let Some(language) = normalize_language_hint(&input.request.language) {
        openai_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if let Some(hotwords) = input
        .request
        .hotwords
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        openai_options.insert(
            "prompt".to_string(),
            serde_json::Value::String(hotwords.replace('\n', ", ")),
        );
    }
    if !openai_options.is_empty() {
        po.insert(
            "openai".to_string(),
            serde_json::Value::Object(openai_options),
        );
    }

    let speaker_diarization_enabled = is_cloud_speaker_diarization_enabled(&input.request);

    // 2. Volcengine options
    let mut volc_options = serde_json::Map::new();
    volc_options.insert(
        "enable_itn".to_string(),
        serde_json::Value::Bool(input.request.enable_itn),
    );
    if speaker_diarization_enabled {
        volc_options.insert(
            "enable_speaker_info".to_string(),
            serde_json::Value::Bool(true),
        );
    }
    if let Some(language) = normalize_language_hint(&input.request.language) {
        volc_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if let Some(hotwords) = input
        .request
        .hotwords
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        volc_options.insert(
            "hotwords".to_string(),
            serde_json::Value::String(hotwords.to_string()),
        );
    }
    po.insert(
        "volcengine".to_string(),
        serde_json::Value::Object(volc_options),
    );

    // 3. Deepgram options
    let mut deepgram_options = serde_json::Map::new();
    if speaker_diarization_enabled {
        deepgram_options.insert("diarize".to_string(), serde_json::Value::Bool(true));
        deepgram_options.insert("paragraphs".to_string(), serde_json::Value::Bool(true));
        deepgram_options.insert("utterances".to_string(), serde_json::Value::Bool(true));
    }
    if let Some(language) = normalize_language_hint(&input.request.language) {
        deepgram_options.insert("language".to_string(), serde_json::Value::String(language));
    }
    if !deepgram_options.is_empty() {
        po.insert(
            "deepgram".to_string(),
            serde_json::Value::Object(deepgram_options),
        );
    }

    // 4. AssemblyAI options
    let mut assemblyai_options = serde_json::Map::new();
    if speaker_diarization_enabled {
        assemblyai_options.insert("speaker_labels".to_string(), serde_json::Value::Bool(true));
    }
    if let Some(language) = normalize_language_hint(&input.request.language) {
        assemblyai_options.insert(
            "language_code".to_string(),
            serde_json::Value::String(language),
        );
    }
    if let Some(hotwords) = input
        .request
        .hotwords
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let words = hotwords
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>();
        if !words.is_empty() {
            assemblyai_options.insert("word_boost".to_string(), serde_json::json!(words));
        }
    }
    if !assemblyai_options.is_empty() {
        po.insert(
            "assemblyai".to_string(),
            serde_json::Value::Object(assemblyai_options),
        );
    }

    // 5. ElevenLabs options
    let mut elevenlabs_options = serde_json::Map::new();
    if speaker_diarization_enabled {
        elevenlabs_options.insert("diarize".to_string(), serde_json::Value::Bool(true));
    }
    if let Some(language) = normalize_language_hint(&input.request.language) {
        elevenlabs_options.insert(
            "languageCode".to_string(),
            serde_json::Value::String(language),
        );
    }
    if !elevenlabs_options.is_empty() {
        po.insert(
            "elevenlabs".to_string(),
            serde_json::Value::Object(elevenlabs_options),
        );
    }

    if !po.is_empty() {
        call_options.provider_options = Some(po);
    }

    let result = model
        .do_generate(&call_options)
        .await
        .map_err(map_aimux_asr_error)?;

    let segments = if speaker_diarization_enabled
        && let Some(raw_body) = result.response.body.as_ref()
        && let Some(parsed) = parse_raw_provider_segments(model.provider(), raw_body)
        && !parsed.is_empty()
    {
        parsed
    } else {
        let mut fallback_segments: Vec<TranscriptSegment> = result
            .segments
            .into_iter()
            .map(|segment| TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text: segment.text.trim().to_string(),
                start: segment.start_second,
                end: segment.end_second,
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            })
            .collect();

        let trimmed_text = result.text.trim();
        if fallback_segments.is_empty() && !trimmed_text.is_empty() {
            let duration = result.duration_in_seconds.unwrap_or(0.0);
            fallback_segments.push(TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text: trimmed_text.to_string(),
                start: 0.0,
                end: duration,
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            });
        }
        fallback_segments
    };

    let audio_duration_ms = result.duration_in_seconds.unwrap_or(0.0) * 1000.0;
    let stage = format!("{}_batch_complete", model.provider().replace('-', "_"));

    Ok(OnlineBatchTranscriptionOutput {
        segments,
        audio_duration_ms,
        buffered_samples: bytes.len() / 2,
        stage,
    })
}

fn append_word_to_sentence(sentence: &mut String, word: &str) {
    let word_trimmed = word.trim();
    if word_trimmed.is_empty() {
        return;
    }
    if sentence.is_empty() {
        sentence.push_str(word_trimmed);
        return;
    }
    let last_char = sentence.chars().last().unwrap_or(' ');
    let first_char = word_trimmed.chars().next().unwrap_or(' ');

    let is_cjk = |c: char| -> bool {
        matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}')
    };

    if (is_cjk(last_char) && is_cjk(first_char))
        || matches!(
            first_char,
            ',' | '.' | '!' | '?' | ';' | ':' | '\'' | ')' | ']' | '}'
        )
    {
        sentence.push_str(word_trimmed);
    } else {
        sentence.push(' ');
        sentence.push_str(word_trimmed);
    }
}

fn extract_speaker_id_from_value(v: &Value) -> Option<String> {
    v.as_str()
        .map(ToString::to_string)
        .or_else(|| v.as_i64().map(|n| n.to_string()))
}

/// Attempt to parse rich provider-specific transcription segments from raw response body.
pub fn parse_raw_provider_segments(
    provider: &str,
    raw_body: &Value,
) -> Option<Vec<TranscriptSegment>> {
    match provider {
        "volcengine" => segments_from_volcengine_response(raw_body, true, "volc-batch").ok(),
        "assemblyai" => parse_assemblyai_raw_segments(raw_body),
        "deepgram" => parse_deepgram_raw_segments(raw_body),
        "elevenlabs" => parse_elevenlabs_raw_segments(raw_body),
        "openai" | "groq" | "mistral" => parse_openai_compatible_raw_segments(raw_body),
        _ => None,
    }
}

/// Parse AssemblyAI raw transcript response containing `utterances`.
pub fn parse_assemblyai_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let utterances = body.get("utterances")?.as_array()?;
    if utterances.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    for (i, u) in utterances.iter().enumerate() {
        let text = u.get("text").and_then(Value::as_str)?.trim();
        if text.is_empty() {
            continue;
        }
        let start = u.get("start").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0;
        let end = u
            .get("end")
            .and_then(Value::as_f64)
            .unwrap_or(start * 1000.0)
            / 1000.0;

        let (speaker, speaker_attribution) = u
            .get("speaker")
            .and_then(Value::as_str)
            .map(create_cloud_speaker)
            .map(|(tag, attr)| (Some(tag), Some(attr)))
            .unwrap_or((None, None));

        let mut timing_units = Vec::new();
        let mut tokens = Vec::new();
        let mut timestamps = Vec::new();
        let mut durations = Vec::new();

        if let Some(words) = u.get("words").and_then(Value::as_array) {
            for w in words {
                let w_text = w
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim();
                if w_text.is_empty() {
                    continue;
                }
                let w_start = w
                    .get("start")
                    .and_then(Value::as_f64)
                    .unwrap_or(start * 1000.0)
                    / 1000.0;
                let w_end = w
                    .get("end")
                    .and_then(Value::as_f64)
                    .unwrap_or(w_start * 1000.0)
                    / 1000.0;

                tokens.push(w_text.to_string());
                timestamps.push(w_start as f32);
                durations.push((w_end.max(w_start) - w_start) as f32);
                timing_units.push(TranscriptTimingUnit {
                    text: w_text.to_string(),
                    start: w_start,
                    end: w_end.max(w_start),
                });
            }
        }

        segments.push(TranscriptSegment {
            id: format!("aai-{i}"),
            text: text.to_string(),
            start,
            end: end.max(start),
            is_final: true,
            timing: (!timing_units.is_empty()).then_some(TranscriptTiming {
                level: TranscriptTimingLevel::Token,
                source: TranscriptTimingSource::Model,
                units: timing_units,
            }),
            tokens: (!tokens.is_empty()).then_some(tokens),
            timestamps: (!timestamps.is_empty()).then_some(timestamps),
            durations: (!durations.is_empty()).then_some(durations),
            translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse Deepgram raw listen response containing `utterances`, `paragraphs`, or word clusters.
pub fn parse_deepgram_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let alt = body
        .get("results")
        .and_then(|r| r.get("channels"))
        .and_then(Value::as_array)
        .and_then(|ch| ch.first())
        .and_then(|c| c.get("alternatives"))
        .and_then(Value::as_array)
        .and_then(|a| a.first());

    // 1. Try utterances
    let utterances = alt
        .and_then(|a| a.get("utterances"))
        .and_then(Value::as_array)
        .or_else(|| {
            body.get("results")
                .and_then(|r| r.get("utterances"))
                .and_then(Value::as_array)
        });

    if let Some(utterances) = utterances
        && !utterances.is_empty()
    {
        let mut segments = Vec::new();
        for (i, u) in utterances.iter().enumerate() {
            let text = u
                .get("transcript")
                .or_else(|| u.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if text.is_empty() {
                continue;
            }
            let start = u.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = u.get("end").and_then(Value::as_f64).unwrap_or(start);

            let speaker_id = u.get("speaker").and_then(extract_speaker_id_from_value);

            let (speaker, speaker_attribution) = match &speaker_id {
                Some(id) => {
                    let (tag, attr) = create_cloud_speaker(id);
                    (Some(tag), Some(attr))
                }
                None => (None, None),
            };

            let mut timing_units = Vec::new();
            let mut tokens = Vec::new();
            let mut timestamps = Vec::new();
            let mut durations = Vec::new();

            if let Some(words) = u.get("words").and_then(Value::as_array) {
                for w in words {
                    let w_text = w
                        .get("punctuated_word")
                        .or_else(|| w.get("word"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if w_text.is_empty() {
                        continue;
                    }
                    let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(start);
                    let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);

                    tokens.push(w_text.to_string());
                    timestamps.push(w_start as f32);
                    durations.push((w_end.max(w_start) - w_start) as f32);
                    timing_units.push(TranscriptTimingUnit {
                        text: w_text.to_string(),
                        start: w_start,
                        end: w_end.max(w_start),
                    });
                }
            }

            segments.push(TranscriptSegment {
                id: format!("dg-utt-{i}"),
                text: text.to_string(),
                start,
                end: end.max(start),
                is_final: true,
                timing: (!timing_units.is_empty()).then_some(TranscriptTiming {
                    level: TranscriptTimingLevel::Token,
                    source: TranscriptTimingSource::Model,
                    units: timing_units,
                }),
                tokens: (!tokens.is_empty()).then_some(tokens),
                timestamps: (!timestamps.is_empty()).then_some(timestamps),
                durations: (!durations.is_empty()).then_some(durations),
                translation: None,
                speaker,
                speaker_attribution,
            });
        }
        if !segments.is_empty() {
            return Some(segments);
        }
    }

    // 2. Try paragraphs
    let paragraphs = alt
        .and_then(|a| a.get("paragraphs"))
        .and_then(|p| p.get("paragraphs"))
        .and_then(Value::as_array);

    if let Some(paragraphs) = paragraphs
        && !paragraphs.is_empty()
    {
        let mut segments = Vec::new();
        for (i, p) in paragraphs.iter().enumerate() {
            let start = p.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = p.get("end").and_then(Value::as_f64).unwrap_or(start);

            let speaker_id = p.get("speaker").and_then(extract_speaker_id_from_value);

            let (speaker, speaker_attribution) = match &speaker_id {
                Some(id) => {
                    let (tag, attr) = create_cloud_speaker(id);
                    (Some(tag), Some(attr))
                }
                None => (None, None),
            };

            let mut text = String::new();
            if let Some(sentences) = p.get("sentences").and_then(Value::as_array) {
                for s in sentences {
                    if let Some(st) = s.get("text").and_then(Value::as_str) {
                        append_word_to_sentence(&mut text, st);
                    }
                }
            }
            if text.trim().is_empty() {
                continue;
            }

            segments.push(TranscriptSegment {
                id: format!("dg-para-{i}"),
                text,
                start,
                end: end.max(start),
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                speaker,
                speaker_attribution,
            });
        }
        if !segments.is_empty() {
            return Some(segments);
        }
    }

    // 3. Fallback: words clustering by speaker
    let words = alt.and_then(|a| a.get("words")).and_then(Value::as_array);
    if let Some(words) = words
        && !words.is_empty()
    {
        let mut segments = Vec::new();
        let mut current_speaker: Option<String> = None;
        let mut current_text = String::new();
        let mut current_start = 0.0;
        let mut current_end = 0.0;
        let mut current_timing = Vec::new();
        let mut current_tokens = Vec::new();
        let mut current_timestamps = Vec::new();
        let mut current_durations = Vec::new();

        let flush_segment = |segments: &mut Vec<TranscriptSegment>,
                             speaker_id: Option<String>,
                             text: &mut String,
                             start: f64,
                             end: f64,
                             timing: &mut Vec<TranscriptTimingUnit>,
                             tokens: &mut Vec<String>,
                             timestamps: &mut Vec<f32>,
                             durations: &mut Vec<f32>| {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let (speaker, speaker_attribution) = match &speaker_id {
                    Some(id) => {
                        let (tag, attr) = create_cloud_speaker(id);
                        (Some(tag), Some(attr))
                    }
                    None => (None, None),
                };
                let idx = segments.len();
                segments.push(TranscriptSegment {
                    id: format!("dg-word-{idx}"),
                    text: trimmed.to_string(),
                    start,
                    end: end.max(start),
                    is_final: true,
                    timing: (!timing.is_empty()).then_some(TranscriptTiming {
                        level: TranscriptTimingLevel::Token,
                        source: TranscriptTimingSource::Model,
                        units: std::mem::take(timing),
                    }),
                    tokens: (!tokens.is_empty()).then_some(std::mem::take(tokens)),
                    timestamps: (!timestamps.is_empty()).then_some(std::mem::take(timestamps)),
                    durations: (!durations.is_empty()).then_some(std::mem::take(durations)),
                    translation: None,
                    speaker,
                    speaker_attribution,
                });
            }
            text.clear();
        };

        for w in words {
            let w_text = w
                .get("punctuated_word")
                .or_else(|| w.get("word"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if w_text.is_empty() {
                continue;
            }
            let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);
            let w_speaker = w.get("speaker").and_then(extract_speaker_id_from_value);

            let speaker_changed = w_speaker != current_speaker;
            let long_pause = !current_text.is_empty() && (w_start - current_end > 3.0);

            if (speaker_changed || long_pause) && !current_text.is_empty() {
                flush_segment(
                    &mut segments,
                    current_speaker.take(),
                    &mut current_text,
                    current_start,
                    current_end,
                    &mut current_timing,
                    &mut current_tokens,
                    &mut current_timestamps,
                    &mut current_durations,
                );
            }

            if current_text.is_empty() {
                current_start = w_start;
                current_speaker = w_speaker;
            }
            current_end = w_end;
            append_word_to_sentence(&mut current_text, w_text);

            current_tokens.push(w_text.to_string());
            current_timestamps.push(w_start as f32);
            current_durations.push((w_end.max(w_start) - w_start) as f32);
            current_timing.push(TranscriptTimingUnit {
                text: w_text.to_string(),
                start: w_start,
                end: w_end.max(w_start),
            });
        }

        if !current_text.is_empty() {
            flush_segment(
                &mut segments,
                current_speaker.take(),
                &mut current_text,
                current_start,
                current_end,
                &mut current_timing,
                &mut current_tokens,
                &mut current_timestamps,
                &mut current_durations,
            );
        }

        if !segments.is_empty() {
            return Some(segments);
        }
    }

    None
}

/// Parse ElevenLabs raw response containing `words` with `speaker_id`.
pub fn parse_elevenlabs_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let words = body.get("words").and_then(Value::as_array)?;
    if words.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    let mut current_speaker: Option<String> = None;
    let mut current_text = String::new();
    let mut current_start = 0.0;
    let mut current_end = 0.0;
    let mut current_timing = Vec::new();
    let mut current_tokens = Vec::new();
    let mut current_timestamps = Vec::new();
    let mut current_durations = Vec::new();

    let flush_segment = |segments: &mut Vec<TranscriptSegment>,
                         speaker_id: Option<String>,
                         text: &mut String,
                         start: f64,
                         end: f64,
                         timing: &mut Vec<TranscriptTimingUnit>,
                         tokens: &mut Vec<String>,
                         timestamps: &mut Vec<f32>,
                         durations: &mut Vec<f32>| {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let (speaker, speaker_attribution) = match &speaker_id {
                Some(id) => {
                    let (tag, attr) = create_cloud_speaker(id);
                    (Some(tag), Some(attr))
                }
                None => (None, None),
            };
            let idx = segments.len();
            segments.push(TranscriptSegment {
                id: format!("el-word-{idx}"),
                text: trimmed.to_string(),
                start,
                end: end.max(start),
                is_final: true,
                timing: (!timing.is_empty()).then_some(TranscriptTiming {
                    level: TranscriptTimingLevel::Token,
                    source: TranscriptTimingSource::Model,
                    units: std::mem::take(timing),
                }),
                tokens: (!tokens.is_empty()).then_some(std::mem::take(tokens)),
                timestamps: (!timestamps.is_empty()).then_some(std::mem::take(timestamps)),
                durations: (!durations.is_empty()).then_some(std::mem::take(durations)),
                translation: None,
                speaker,
                speaker_attribution,
            });
        }
        text.clear();
    };

    for w in words {
        let w_text = w
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if w_text.is_empty() {
            continue;
        }
        let w_start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let w_end = w.get("end").and_then(Value::as_f64).unwrap_or(w_start);
        let w_speaker = w.get("speaker_id").and_then(extract_speaker_id_from_value);

        let speaker_changed = w_speaker != current_speaker;
        let long_pause = !current_text.is_empty() && (w_start - current_end > 3.0);

        if (speaker_changed || long_pause) && !current_text.is_empty() {
            flush_segment(
                &mut segments,
                current_speaker.take(),
                &mut current_text,
                current_start,
                current_end,
                &mut current_timing,
                &mut current_tokens,
                &mut current_timestamps,
                &mut current_durations,
            );
        }

        if current_text.is_empty() {
            current_start = w_start;
            current_speaker = w_speaker;
        }
        current_end = w_end;
        append_word_to_sentence(&mut current_text, w_text);

        current_tokens.push(w_text.to_string());
        current_timestamps.push(w_start as f32);
        current_durations.push((w_end.max(w_start) - w_start) as f32);
        current_timing.push(TranscriptTimingUnit {
            text: w_text.to_string(),
            start: w_start,
            end: w_end.max(w_start),
        });
    }

    if !current_text.is_empty() {
        flush_segment(
            &mut segments,
            current_speaker.take(),
            &mut current_text,
            current_start,
            current_end,
            &mut current_timing,
            &mut current_tokens,
            &mut current_timestamps,
            &mut current_durations,
        );
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

/// Parse OpenAI / Mistral compatible response containing `segments` with `speaker` field.
pub fn parse_openai_compatible_raw_segments(body: &Value) -> Option<Vec<TranscriptSegment>> {
    let raw_segments = body.get("segments").and_then(Value::as_array)?;
    if raw_segments.is_empty() {
        return None;
    }

    let has_speaker = raw_segments
        .iter()
        .any(|s| s.get("speaker").is_some() || s.get("speaker_id").is_some());
    if !has_speaker {
        return None;
    }

    let mut segments = Vec::new();
    for (i, s) in raw_segments.iter().enumerate() {
        let text = s.get("text").and_then(Value::as_str)?.trim();
        let start = s.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let end = s.get("end").and_then(Value::as_f64).unwrap_or(start);

        let speaker_id = s
            .get("speaker")
            .or_else(|| s.get("speaker_id"))
            .and_then(extract_speaker_id_from_value);

        let (speaker, speaker_attribution) = match &speaker_id {
            Some(id) => {
                let (tag, attr) = create_cloud_speaker(id);
                (Some(tag), Some(attr))
            }
            None => (None, None),
        };

        segments.push(TranscriptSegment {
            id: format!("openai-seg-{i}"),
            text: text.to_string(),
            start,
            end: end.max(start),
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker,
            speaker_attribution,
        });
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aimux_core::error::{AiMuxError, ApiCallError};
    use serde_json::json;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, AsrPortErrorKind, AsrTranscriptionRequest,
        OnlineAsrProviderRequest,
    };
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };

    fn online_request(provider_id: &str, config: serde_json::Value) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode: AsrMode::Batch,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.to_string(),
                    profile_id: format!("{provider_id}-default"),
                    config,
                },
            },
        }
    }

    #[test]
    fn detects_audio_mime_types() {
        assert_eq!(detect_audio_mime_type(Path::new("test.wav")), "audio/wav");
        assert_eq!(detect_audio_mime_type(Path::new("test.mp3")), "audio/mp3");
        assert_eq!(detect_audio_mime_type(Path::new("test.ogg")), "audio/ogg");
        assert_eq!(detect_audio_mime_type(Path::new("test.flac")), "audio/flac");
        assert_eq!(detect_audio_mime_type(Path::new("test.m4a")), "audio/m4a");
        assert_eq!(
            detect_audio_mime_type(Path::new("test.unknown")),
            "audio/wav"
        );
    }

    #[test]
    fn creates_aimux_model_for_openai_whisper() {
        let request = online_request(
            OPENAI_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "test-openai-key",
                "model": "whisper-1"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "openai");
        assert_eq!(model.model_id(), "whisper-1");
    }

    #[test]
    fn creates_aimux_model_for_groq_whisper() {
        let request = online_request(
            GROQ_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "test-groq-key",
                "model": "whisper-large-v3-turbo"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "groq");
        assert_eq!(model.model_id(), "whisper-large-v3-turbo");
    }

    #[test]
    fn creates_aimux_model_for_mistral_voxtral() {
        let request = online_request(
            MISTRAL_VOXTRAL_PROVIDER_ID,
            json!({
                "apiKey": "test-mistral-key",
                "model": "mistral-small-latest"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "mistral");
        assert_eq!(model.model_id(), "mistral-small-latest");
    }

    #[test]
    fn creates_aimux_model_for_deepgram() {
        let request = online_request(
            DEEPGRAM_PROVIDER_ID,
            json!({
                "apiKey": "test-deepgram-key",
                "model": "nova-2"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "deepgram");
        assert_eq!(model.model_id(), "nova-2");
    }

    #[test]
    fn creates_aimux_model_for_assemblyai() {
        let request = online_request(
            ASSEMBLYAI_PROVIDER_ID,
            json!({
                "apiKey": "test-aai-key",
                "model": "best"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "assemblyai");
        assert_eq!(model.model_id(), "best");
    }

    #[test]
    fn creates_aimux_model_for_elevenlabs() {
        let request = online_request(
            ELEVENLABS_PROVIDER_ID,
            json!({
                "apiKey": "test-el-key",
                "model": "scribe_v1"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "elevenlabs");
        assert_eq!(model.model_id(), "scribe_v1");
    }

    #[test]
    fn creates_aimux_model_for_volcengine_doubao() {
        let request = online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            json!({
                "apiKey": "test-volc-key",
                "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash",
                "batchResourceId": "volc.bigasr.auc_turbo"
            }),
        );

        let model = create_aimux_transcription_model(&request).unwrap();
        assert_eq!(model.provider(), "volcengine");
        assert_eq!(model.model_id(), "volc.bigasr.auc_turbo");
    }

    #[test]
    fn missing_api_key_fails_authentication() {
        let request = online_request(
            OPENAI_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": "   "
            }),
        );

        let err = match create_aimux_transcription_model(&request) {
            Err(err) => err,
            Ok(_) => panic!("expected missing API key error"),
        };
        assert_eq!(err.kind, AsrPortErrorKind::Authentication);
    }

    #[test]
    fn maps_aimux_errors_to_typed_asr_port_errors() {
        let auth_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(401),
            provider_code: None,
            message: "Unauthorized".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: false,
        });
        assert_eq!(
            map_aimux_asr_error(auth_error).kind,
            AsrPortErrorKind::Authentication
        );

        let rate_limit_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(429),
            provider_code: None,
            message: "Rate limit reached".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: Some(1000),
            is_retryable: true,
        });
        assert_eq!(
            map_aimux_asr_error(rate_limit_error).kind,
            AsrPortErrorKind::RateLimited
        );

        let unavailable_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(503),
            provider_code: None,
            message: "Service Unavailable".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: true,
        });
        assert_eq!(
            map_aimux_asr_error(unavailable_error).kind,
            AsrPortErrorKind::Unavailable
        );

        let timeout_error = AiMuxError::Timeout("connection timed out".to_string());
        assert_eq!(
            map_aimux_asr_error(timeout_error).kind,
            AsrPortErrorKind::Timeout
        );

        let not_found_error = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(404),
            provider_code: None,
            message: "Not found".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: false,
        });
        assert_eq!(
            map_aimux_asr_error(not_found_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let payload_too_large = AiMuxError::ApiCall(ApiCallError {
            status_code: Some(413),
            provider_code: None,
            message: "Payload Too Large".to_string(),
            response_body: None,
            request_id: None,
            retry_after_ms: None,
            is_retryable: false,
        });
        assert_eq!(
            map_aimux_asr_error(payload_too_large).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let invalid_arg_error = AiMuxError::InvalidArgument("bad param".to_string());
        assert_eq!(
            map_aimux_asr_error(invalid_arg_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let no_model_error = AiMuxError::NoSuchModel {
            model_id: "whisper-nonexistent".to_string(),
            model_type: "transcriptionModel".to_string(),
        };
        assert_eq!(
            map_aimux_asr_error(no_model_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let no_provider_error = AiMuxError::NoSuchProvider {
            provider_id: "unknown".to_string(),
        };
        assert_eq!(
            map_aimux_asr_error(no_provider_error).kind,
            AsrPortErrorKind::InvalidRequest
        );

        let unsupported_error =
            AiMuxError::UnsupportedFunctionality("streaming not supported".to_string());
        assert_eq!(
            map_aimux_asr_error(unsupported_error).kind,
            AsrPortErrorKind::Unsupported
        );

        let parse_error = AiMuxError::JsonParse("failed to parse".to_string());
        assert_eq!(
            map_aimux_asr_error(parse_error).kind,
            AsrPortErrorKind::Protocol
        );
    }

    #[test]
    fn test_create_cloud_speaker_labels() {
        let (tag0, attr0) = create_cloud_speaker("0");
        assert_eq!(tag0.id, "cloud-speaker-0");
        assert_eq!(tag0.label, "Speaker 1");
        assert_eq!(tag0.kind, "anonymous");
        assert_eq!(attr0.group_id, "cloud-speaker-0");
        assert_eq!(attr0.anonymous_label, "Speaker 1");
        assert_eq!(attr0.source, "cloud");

        let (tag1, attr1) = create_cloud_speaker("speaker_1");
        assert_eq!(tag1.id, "cloud-speaker-speaker_1");
        assert_eq!(tag1.label, "Speaker 2");
        assert_eq!(attr1.anonymous_label, "Speaker 2");

        let (tag_a, attr_a) = create_cloud_speaker("A");
        assert_eq!(tag_a.id, "cloud-speaker-A");
        assert_eq!(tag_a.label, "Speaker A");
        assert_eq!(attr_a.anonymous_label, "Speaker A");
    }

    #[test]
    fn test_is_cloud_speaker_diarization_enabled() {
        let req_default = online_request(VOLCENGINE_DOUBAO_PROVIDER_ID, json!({ "apiKey": "k" }));
        assert!(is_cloud_speaker_diarization_enabled(&req_default));

        let req_explicit_true = online_request(
            DEEPGRAM_PROVIDER_ID,
            json!({ "apiKey": "k", "speakerDiarization": true }),
        );
        assert!(is_cloud_speaker_diarization_enabled(&req_explicit_true));

        let req_disabled = online_request(
            DEEPGRAM_PROVIDER_ID,
            json!({ "apiKey": "k", "speakerDiarization": false }),
        );
        assert!(!is_cloud_speaker_diarization_enabled(&req_disabled));

        let req_disabled_snake = online_request(
            ASSEMBLYAI_PROVIDER_ID,
            json!({ "apiKey": "k", "speaker_diarization": false }),
        );
        assert!(!is_cloud_speaker_diarization_enabled(&req_disabled_snake));
    }

    #[test]
    fn test_parse_assemblyai_raw_segments() {
        let raw = json!({
            "status": "completed",
            "utterances": [
                {
                    "start": 0,
                    "end": 1500,
                    "speaker": "A",
                    "text": "Hello world.",
                    "words": [
                        { "start": 0, "end": 500, "text": "Hello", "confidence": 0.98, "speaker": "A" },
                        { "start": 550, "end": 1500, "text": "world.", "confidence": 0.99, "speaker": "A" }
                    ]
                },
                {
                    "start": 1600,
                    "end": 3000,
                    "speaker": "B",
                    "text": "Good morning.",
                    "words": [
                        { "start": 1600, "end": 2200, "text": "Good", "confidence": 0.95, "speaker": "B" },
                        { "start": 2250, "end": 3000, "text": "morning.", "confidence": 0.97, "speaker": "B" }
                    ]
                }
            ]
        });

        let segments = parse_assemblyai_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 2);

        assert_eq!(segments[0].id, "aai-0");
        assert_eq!(segments[0].text, "Hello world.");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 1.5);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker A");
        assert_eq!(segments[0].speaker.as_ref().unwrap().id, "cloud-speaker-A");
        assert_eq!(
            segments[0].tokens.as_ref().unwrap(),
            &vec!["Hello", "world."]
        );
        assert_eq!(segments[0].timestamps.as_ref().unwrap(), &vec![0.0, 0.55]);

        assert_eq!(segments[1].id, "aai-1");
        assert_eq!(segments[1].text, "Good morning.");
        assert_eq!(segments[1].start, 1.6);
        assert_eq!(segments[1].end, 3.0);
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker B");
    }

    #[test]
    fn test_parse_deepgram_raw_segments_utterances() {
        let raw = json!({
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "transcript": "Hello from Deepgram.",
                                "utterances": [
                                    {
                                        "start": 0.0,
                                        "end": 2.0,
                                        "transcript": "Hello from Deepgram.",
                                        "speaker": 0,
                                        "words": [
                                            { "punctuated_word": "Hello", "start": 0.0, "end": 0.6, "speaker": 0 },
                                            { "punctuated_word": "from", "start": 0.7, "end": 1.0, "speaker": 0 },
                                            { "punctuated_word": "Deepgram.", "start": 1.1, "end": 2.0, "speaker": 0 }
                                        ]
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        });

        let segments = parse_deepgram_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "dg-utt-0");
        assert_eq!(segments[0].text, "Hello from Deepgram.");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 2.0);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 1");
        assert_eq!(segments[0].speaker.as_ref().unwrap().id, "cloud-speaker-0");
    }

    #[test]
    fn test_parse_deepgram_raw_segments_paragraphs() {
        let raw = json!({
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "paragraphs": {
                                    "paragraphs": [
                                        {
                                            "speaker": 1,
                                            "start": 1.0,
                                            "end": 3.5,
                                            "sentences": [
                                                { "text": "Paragraph one.", "start": 1.0, "end": 2.0 },
                                                { "text": "Second sentence.", "start": 2.1, "end": 3.5 }
                                            ]
                                        }
                                    ]
                                }
                            }
                        ]
                    }
                ]
            }
        });

        let segments = parse_deepgram_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "dg-para-0");
        assert_eq!(segments[0].text, "Paragraph one. Second sentence.");
        assert_eq!(segments[0].start, 1.0);
        assert_eq!(segments[0].end, 3.5);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 2");
        assert_eq!(segments[0].speaker.as_ref().unwrap().id, "cloud-speaker-1");
    }

    #[test]
    fn test_parse_deepgram_raw_segments_words() {
        let raw = json!({
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "words": [
                                    { "punctuated_word": "First", "start": 0.0, "end": 0.5, "speaker": 0 },
                                    { "punctuated_word": "speaker.", "start": 0.6, "end": 1.0, "speaker": 0 },
                                    { "punctuated_word": "Second", "start": 1.5, "end": 2.0, "speaker": 1 },
                                    { "punctuated_word": "speaker.", "start": 2.1, "end": 2.5, "speaker": 1 }
                                ]
                            }
                        ]
                    }
                ]
            }
        });

        let segments = parse_deepgram_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "First speaker.");
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 1");
        assert_eq!(segments[1].text, "Second speaker.");
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker 2");
    }

    #[test]
    fn test_parse_elevenlabs_raw_segments() {
        let raw = json!({
            "text": "Hello, how are you? I'm fine.",
            "words": [
                { "text": "Hello,", "start": 0.0, "end": 0.5, "speaker_id": "speaker_0" },
                { "text": "how", "start": 0.6, "end": 0.8, "speaker_id": "speaker_0" },
                { "text": "are", "start": 0.9, "end": 1.0, "speaker_id": "speaker_0" },
                { "text": "you?", "start": 1.1, "end": 1.5, "speaker_id": "speaker_0" },
                { "text": "I'm", "start": 2.0, "end": 2.4, "speaker_id": "speaker_1" },
                { "text": "fine.", "start": 2.5, "end": 2.9, "speaker_id": "speaker_1" }
            ]
        });

        let segments = parse_elevenlabs_raw_segments(&raw).expect("parsed segments");
        assert_eq!(segments.len(), 2);

        assert_eq!(segments[0].text, "Hello, how are you?");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 1.5);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker 1");
        assert_eq!(
            segments[0].speaker.as_ref().unwrap().id,
            "cloud-speaker-speaker_0"
        );

        assert_eq!(segments[1].text, "I'm fine.");
        assert_eq!(segments[1].start, 2.0);
        assert_eq!(segments[1].end, 2.9);
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker 2");
        assert_eq!(
            segments[1].speaker.as_ref().unwrap().id,
            "cloud-speaker-speaker_1"
        );
    }

    #[test]
    fn test_parse_openai_compatible_raw_segments() {
        let raw_with_speaker = json!({
            "segments": [
                {
                    "id": 0,
                    "start": 0.0,
                    "end": 2.0,
                    "text": "Hello from speaker A",
                    "speaker": "A"
                },
                {
                    "id": 1,
                    "start": 2.5,
                    "end": 4.0,
                    "text": "Hello from speaker B",
                    "speaker": "B"
                }
            ]
        });

        let segments =
            parse_openai_compatible_raw_segments(&raw_with_speaker).expect("parsed segments");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker.as_ref().unwrap().label, "Speaker A");
        assert_eq!(segments[1].speaker.as_ref().unwrap().label, "Speaker B");

        let raw_without_speaker = json!({
            "segments": [
                {
                    "id": 0,
                    "start": 0.0,
                    "end": 2.0,
                    "text": "No speaker info here"
                }
            ]
        });
        assert!(parse_openai_compatible_raw_segments(&raw_without_speaker).is_none());
    }

    #[test]
    fn test_append_word_to_sentence() {
        let mut sentence = String::new();
        append_word_to_sentence(&mut sentence, "Hello");
        append_word_to_sentence(&mut sentence, ",");
        append_word_to_sentence(&mut sentence, "world");
        append_word_to_sentence(&mut sentence, "!");
        assert_eq!(sentence, "Hello, world!");

        let mut cjk = String::new();
        append_word_to_sentence(&mut cjk, "你好");
        append_word_to_sentence(&mut cjk, "世界");
        assert_eq!(cjk, "你好世界");
    }
}
