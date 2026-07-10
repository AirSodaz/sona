pub(crate) mod streaming;
mod transcript;

use crate::{VolcengineConfigError, VolcengineServerFrameError};
use sona_core::ports::asr::SherpaError;

impl From<VolcengineConfigError> for SherpaError {
    fn from(error: VolcengineConfigError) -> Self {
        match error {
            VolcengineConfigError::ProviderConfigMissing => Self::VolcengineProviderConfigMissing,
            VolcengineConfigError::UnsupportedProvider { provider_id } => {
                Self::UnsupportedVolcengineProvider { provider_id }
            }
            VolcengineConfigError::ApiKeyMissing => Self::VolcengineApiKeyMissing,
            VolcengineConfigError::StreamingConfigMissing => Self::VolcengineStreamingConfigMissing,
            VolcengineConfigError::BatchConfigMissing => Self::VolcengineBatchConfigMissing,
            VolcengineConfigError::LocalFileBatchUnsupported => {
                Self::VolcengineLocalFileBatchUnsupported {
                    message: error.to_string(),
                }
            }
            VolcengineConfigError::ManifestMissing
            | VolcengineConfigError::ManifestDefaultsInvalid => Self::Generic(error.to_string()),
        }
    }
}

impl From<VolcengineServerFrameError> for SherpaError {
    fn from(error: VolcengineServerFrameError) -> Self {
        match error {
            VolcengineServerFrameError::FrameTooShort => Self::VolcengineFrameTooShort,
            VolcengineServerFrameError::ErrorFrame => Self::VolcengineErrorFrame,
            VolcengineServerFrameError::ErrorCodeParseFailed => {
                Self::VolcengineErrorCodeParseFailed
            }
            VolcengineServerFrameError::ErrorLengthParseFailed => {
                Self::VolcengineErrorLengthParseFailed
            }
            VolcengineServerFrameError::ApiError { code, message } => {
                Self::VolcengineApiError { code, message }
            }
            VolcengineServerFrameError::PayloadLengthMissing => {
                Self::VolcenginePayloadLengthMissing
            }
            VolcengineServerFrameError::PayloadLengthParseFailed => {
                Self::VolcenginePayloadLengthParseFailed
            }
            VolcengineServerFrameError::PayloadIncomplete => Self::VolcenginePayloadIncomplete,
            VolcengineServerFrameError::ResponseParseFailed { error } => {
                Self::VolcengineResponseParseFailed { error }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::transcript::*;
    use crate::{VolcengineConfigError, VolcengineServerFrameError};
    use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent, SherpaError};
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::postprocess::TranscriptNormalizationOptions;
    use sona_core::transcription::transcript::TranscriptSegment;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<AsrTranscriptUpdateEvent>>,
    }

    impl AsrRuntimeObserver for RecordingObserver {
        fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
            self.events.lock().unwrap().push(event.clone());
        }

        fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

        fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
    }

    #[test]
    fn config_errors_map_to_the_existing_core_error_contract() {
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::ProviderConfigMissing),
            SherpaError::VolcengineProviderConfigMissing
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::UnsupportedProvider {
                provider_id: "custom-volcengine".to_string(),
            }),
            SherpaError::UnsupportedVolcengineProvider { provider_id }
                if provider_id == "custom-volcengine"
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::ApiKeyMissing),
            SherpaError::VolcengineApiKeyMissing
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::StreamingConfigMissing),
            SherpaError::VolcengineStreamingConfigMissing
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::BatchConfigMissing),
            SherpaError::VolcengineBatchConfigMissing
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::LocalFileBatchUnsupported),
            SherpaError::VolcengineLocalFileBatchUnsupported { message }
                if message == "Volcengine local file batch supports only recognize/flash endpoints."
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::ManifestMissing),
            SherpaError::Generic(message)
                if message == "Volcengine Doubao provider not found in manifest"
        ));
        assert!(matches!(
            SherpaError::from(VolcengineConfigError::ManifestDefaultsInvalid),
            SherpaError::Generic(message)
                if message == "Volcengine Doubao provider defaults should be an object"
        ));
    }

    #[test]
    fn server_frame_errors_map_to_the_existing_core_error_contract() {
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::FrameTooShort),
            SherpaError::VolcengineFrameTooShort
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::ErrorFrame),
            SherpaError::VolcengineErrorFrame
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::ErrorCodeParseFailed),
            SherpaError::VolcengineErrorCodeParseFailed
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::ErrorLengthParseFailed),
            SherpaError::VolcengineErrorLengthParseFailed
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::ApiError {
                code: 45000001,
                message: "bad request".to_string(),
            }),
            SherpaError::VolcengineApiError { code, message }
                if code == 45000001 && message == "bad request"
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::PayloadLengthMissing),
            SherpaError::VolcenginePayloadLengthMissing
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::PayloadLengthParseFailed),
            SherpaError::VolcenginePayloadLengthParseFailed
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::PayloadIncomplete),
            SherpaError::VolcenginePayloadIncomplete
        ));
        assert!(matches!(
            SherpaError::from(VolcengineServerFrameError::ResponseParseFailed {
                error: "invalid json".to_string(),
            }),
            SherpaError::VolcengineResponseParseFailed { error }
                if error == "invalid json"
        ));
    }

    #[test]
    fn normalization_splits_final_segments_with_generated_ids() {
        let segment = TranscriptSegment {
            id: "segment-1".to_string(),
            text: "Hello. World.".to_string(),
            start: 0.0,
            end: 2.0,
            is_final: true,
            timing: None,
            tokens: Some(vec![
                "Hello".to_string(),
                ".".to_string(),
                "World".to_string(),
                ".".to_string(),
            ]),
            timestamps: Some(vec![0.0, 0.5, 1.0, 1.5]),
            durations: Some(vec![0.5; 4]),
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };

        let segments = normalize_segments(
            vec![segment],
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Hello.");
        assert_eq!(segments[1].text, "World.");
        assert_eq!(segments[0].id, "segment-1");
        assert_ne!(segments[1].id, "segment-1");
        assert!(uuid::Uuid::parse_str(&segments[1].id).is_ok());
    }

    #[test]
    fn observer_receives_the_typed_volcengine_update() {
        let observer = RecordingObserver::default();
        let segment = TranscriptSegment {
            id: "segment-1".to_string(),
            text: "hello".to_string(),
            start: 0.0,
            end: 1.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };

        let update = build_transcript_update(segment, TranscriptNormalizationOptions::default());
        observe_transcript_update(&observer, "live-1", &update);

        assert_eq!(
            *observer.events.lock().unwrap(),
            vec![AsrTranscriptUpdateEvent {
                instance_id: "live-1".to_string(),
                stage: "volcengine_streaming".to_string(),
                update,
            }]
        );
    }
}
