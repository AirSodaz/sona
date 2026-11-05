use crate::runners::VadModelRunner;
use crate::runners::fsmn::FsmnVadRunner;
use crate::runners::silero::SileroVadRunner;
use crate::shared::{SUPPORTED_SAMPLE_RATE, reject_unsupported_rate, resolve_model_onnx_path};
use crate::state_machine::UniversalVadStateMachine;
use crate::streaming::NativeStreamingVad;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::ports::vad::{
    SpeechSpan, StreamingVadPort, VadDetectionOptions, VadEngineKind, VadEnginePort,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

static SESSION_CACHE: LazyLock<Mutex<HashMap<PathBuf, Arc<Session>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Prunes idle VAD sessions from the global cache.
///
/// Retains any session currently held by an active runner or stream (`Arc::strong_count > 1`)
/// or matching the designated active model path.
pub fn prune_idle_sessions(active_path: Option<&Path>) {
    let mut cache = SESSION_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let active_canonical = active_path
        .map(|p| resolve_model_onnx_path(p).unwrap_or_else(|_| p.to_path_buf()))
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));
    cache.retain(|path, session| {
        if let Some(ref active) = active_canonical
            && path == active
        {
            return true;
        }
        Arc::strong_count(session) > 1
    });
}

/// Clears all cached ONNX sessions unconditionally.
pub fn clear_session_cache() {
    let mut cache = SESSION_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.clear();
}

/// Returns the current number of cached sessions.
pub fn cached_session_count() -> usize {
    SESSION_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len()
}

/// Returns the current number of cached sessions matching a predicate.
pub fn cached_session_count_matching<F: Fn(&Path) -> bool>(predicate: F) -> usize {
    SESSION_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .keys()
        .filter(|p| predicate(p))
        .count()
}

pub(crate) fn get_or_create_session(path: &Path) -> Result<Arc<Session>, AsrPortError> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    {
        let cache = SESSION_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(session) = cache.get(&canonical) {
            return Ok(session.clone());
        }
    }

    let session = Session::builder()
        .map_err(|e| AsrPortError::new(AsrPortErrorKind::Model, e.to_string()))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| AsrPortError::new(AsrPortErrorKind::Model, e.to_string()))?
        .with_intra_threads(1)
        .map_err(|e| AsrPortError::new(AsrPortErrorKind::Model, e.to_string()))?
        .commit_from_file(&canonical)
        .map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!("Failed to load VAD model from {}: {e}", path.display()),
            )
        })?;

    let session_arc = Arc::new(session);
    let mut cache = SESSION_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let entry = cache.entry(canonical).or_insert(session_arc);
    Ok(entry.clone())
}

/// Silero VAD engine running on native ONNX Runtime.
///
/// Supports Silero V4, V5, and V6 models with automatic state node and dimension detection.
/// Acts as the default fallback for custom ONNX models.
#[derive(Debug, Clone, Copy, Default)]
pub struct SileroOnnxEngine;

impl VadEnginePort for SileroOnnxEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::SileroOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        !is_fsmn_vad_model_path(model_path)
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        reject_unsupported_rate("SileroOnnxEngine", sample_rate)?;
        reject_unsupported_rate("SileroOnnxEngine", options.sample_rate)?;
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let session = get_or_create_session(&onnx_path)?;
        let mut runner = SileroVadRunner::from_session(session)?;

        let probs = runner.predict_probs_batch(samples)?;
        let mut batch_options = options.clone();
        batch_options.speech_pad_seconds = 0.0;
        let mut state_machine = UniversalVadStateMachine::new(&batch_options, sample_rate);
        let mut spans = state_machine.batch_detect(&probs, runner.window_size());
        for span in &mut spans {
            span.start_sample = span.start_sample.min(samples.len());
            span.end_sample = span.end_sample.min(samples.len());
        }
        spans.retain(|s| s.start_sample < s.end_sample);
        Ok(spans)
    }

    fn create_stream_detector(
        &self,
        options: &VadDetectionOptions,
    ) -> Result<Box<dyn StreamingVadPort>, AsrPortError> {
        reject_unsupported_rate("SileroOnnxEngine", options.sample_rate)?;
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let session = get_or_create_session(&onnx_path)?;
        let runner = SileroVadRunner::from_session(session)?;
        Ok(Box::new(NativeStreamingVad::new(
            Box::new(runner),
            options,
            SUPPORTED_SAMPLE_RATE,
        )))
    }

    fn prune_idle(&self, active_path: Option<&Path>) {
        prune_idle_sessions(active_path);
    }

    fn clear_cache(&self) {
        clear_session_cache();
    }

    fn cached_count(&self) -> usize {
        cached_session_count_matching(|p| self.can_handle(p))
    }
}

/// Alibaba FunASR FSMN-VAD engine running on native ONNX Runtime.
///
/// Uses an embedded 80-dim log-mel filterbank frontend and context cache for streaming
/// and single-call vectorized evaluation for batch processing.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsmnVadOnnxEngine;

impl VadEnginePort for FsmnVadOnnxEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::FsmnVadOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        is_fsmn_vad_model_path(model_path)
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        reject_unsupported_rate("FsmnVadOnnxEngine", sample_rate)?;
        reject_unsupported_rate("FsmnVadOnnxEngine", options.sample_rate)?;
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let session = get_or_create_session(&onnx_path)?;
        let mut runner = FsmnVadRunner::from_session(session, Some(&onnx_path))?;

        let probs = runner.predict_probs_batch(samples)?;
        let mut batch_options = options.clone();
        batch_options.speech_pad_seconds = 0.0;
        let mut state_machine = UniversalVadStateMachine::new(&batch_options, sample_rate);
        let mut spans = state_machine.batch_detect(&probs, runner.window_size());
        for span in &mut spans {
            span.start_sample = span.start_sample.min(samples.len());
            span.end_sample = span.end_sample.min(samples.len());
        }
        spans.retain(|s| s.start_sample < s.end_sample);
        Ok(spans)
    }

    fn create_stream_detector(
        &self,
        options: &VadDetectionOptions,
    ) -> Result<Box<dyn StreamingVadPort>, AsrPortError> {
        reject_unsupported_rate("FsmnVadOnnxEngine", options.sample_rate)?;
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let session = get_or_create_session(&onnx_path)?;
        let runner = FsmnVadRunner::from_session(session, Some(&onnx_path))?;
        Ok(Box::new(NativeStreamingVad::new(
            Box::new(runner),
            options,
            SUPPORTED_SAMPLE_RATE,
        )))
    }

    fn prune_idle(&self, active_path: Option<&Path>) {
        prune_idle_sessions(active_path);
    }

    fn clear_cache(&self) {
        clear_session_cache();
    }

    fn cached_count(&self) -> usize {
        cached_session_count_matching(|p| self.can_handle(p))
    }
}

pub(crate) fn is_fsmn_vad_model_path(model_path: &Path) -> bool {
    let check_str = |s: &str| s.to_ascii_lowercase().contains("fsmn");

    let check_silero = |s: &str| s.to_ascii_lowercase().contains("silero");
    if model_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(check_silero)
    {
        return false;
    }
    let parent = model_path.parent().unwrap_or_else(|| Path::new(""));
    if !parent.as_os_str().is_empty()
        && parent
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(check_silero)
    {
        return false;
    }

    if model_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(check_str)
    {
        return true;
    }

    // parent already resolved above
    let parent_matches = !parent.as_os_str().is_empty()
        && parent
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(check_str);
    let sibling_mvn = model_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(|name| name == "model.onnx" || name.ends_with(".onnx"))
        && (parent.join("am.mvn").is_file() || parent.join("vad.mvn").is_file());

    if parent_matches || sibling_mvn {
        return true;
    }
    if model_path.is_dir()
        && let Ok(entries) = std::fs::read_dir(model_path)
    {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && ((check_str(name) && name.ends_with(".onnx"))
                    || name == "am.mvn"
                    || name == "vad.mvn")
            {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_kind_and_dispatch_routing() {
        let silero = SileroOnnxEngine;
        let fsmn = FsmnVadOnnxEngine;

        assert_eq!(silero.engine_kind(), VadEngineKind::SileroOnnx);
        assert_eq!(fsmn.engine_kind(), VadEngineKind::FsmnVadOnnx);

        assert!(fsmn.can_handle(Path::new("models/fsmn_vad.onnx")));
        assert!(fsmn.can_handle(Path::new("models/speech_fsmn_vad_zh-cn-16k-common.onnx")));
        assert!(fsmn.can_handle(Path::new("speech_fsmn_vad_zh-cn-16k-common/model.onnx")));
        assert!(!fsmn.can_handle(Path::new("models/silero_vad.onnx")));
        assert!(!fsmn.can_handle(Path::new("models/silero/model.onnx")));
        assert!(fsmn.can_handle(Path::new("/home/silero/models/fsmn_vad.onnx")));
        assert!(fsmn.can_handle(Path::new("D:/projects/silero_experiment/fsmn/model.onnx")));
        assert!(silero.can_handle(Path::new("models/silero_vad.onnx")));
        assert!(silero.can_handle(Path::new("models/silero_vad_v5.onnx")));
        assert!(silero.can_handle(Path::new("models/silero_vad_v6.onnx")));
        assert!(!silero.can_handle(Path::new("models/fsmn_vad.onnx")));
        // Fallback for custom onnx models
        assert!(silero.can_handle(Path::new("models/custom_vad_model.onnx")));
    }

    #[test]
    fn detect_rejects_unsupported_sample_rate() {
        let silero = SileroOnnxEngine;
        let mut opts = VadDetectionOptions::batch_defaults("mock.onnx");
        let err = silero.detect(&[0.0; 1600], 44_100, &opts).unwrap_err();
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);

        opts.sample_rate = 8_000;
        let err = silero.detect(&[0.0; 1600], 16_000, &opts).unwrap_err();
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);

        let err = match silero.create_stream_detector(&opts) {
            Err(e) => e,
            Ok(_) => panic!("expected error for unsupported sample rate"),
        };
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);
    }

    #[test]
    fn session_cache_returns_identical_arc_pointer() {
        let silero_candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../models/silero_vad.onnx"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../models/silero_vad_v5.onnx"),
        ];
        if let Some(path) = silero_candidates.iter().find(|p| p.is_file()) {
            let s1 = get_or_create_session(path).expect("session 1 loads");
            let s2 = get_or_create_session(path).expect("session 2 loads");
            assert!(
                Arc::ptr_eq(&s1, &s2),
                "Session Arc pointer should be identical across calls"
            );
        }
    }

    #[test]
    fn test_session_cache_prune_and_clear() {
        let silero_candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../models/silero_vad.onnx"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../models/silero_vad_v5.onnx"),
        ];
        if let Some(path) = silero_candidates.iter().find(|p| p.is_file()) {
            let session = get_or_create_session(path).expect("session loads");
            assert!(cached_session_count() >= 1);

            // Active path retention
            prune_idle_sessions(Some(path));
            assert!(cached_session_count() >= 1);

            // Strong count retention when held by runner/external Arc
            drop(session); // Now only held by cache (strong_count == 1)

            // Pruning without active path removes idle session
            prune_idle_sessions(None);
            assert_eq!(cached_session_count(), 0);

            // Repopulate and clear
            let _ = get_or_create_session(path);
            assert!(cached_session_count() >= 1);
            clear_session_cache();
            assert_eq!(cached_session_count(), 0);
        }
    }
}
