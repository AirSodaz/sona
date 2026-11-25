pub mod asr;
pub mod audio;
pub mod emby;
pub mod llm;
pub mod media_server;
pub mod models;
pub mod translation;

pub mod streaming;

static SHARED_ORT_ENV: std::sync::Mutex<Option<sona_vad::OrtEnvironment>> =
    std::sync::Mutex::new(None);

fn desktop_shared_ort_environment() -> sona_vad::OrtResult<sona_vad::OrtEnvironment> {
    let mut guard = SHARED_ORT_ENV.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(env) = guard.as_ref() {
        return Ok(env.clone());
    }
    let env = sona_vad::create_ort_environment()?;
    *guard = Some(env.clone());
    Ok(env)
}

/// Ensures `sona-vad` and `sona-translation-onnx` share a single process-wide ORT environment.
pub(crate) fn register_shared_ort_environment() {
    sona_vad::register_ort_environment_provider(desktop_shared_ort_environment);
    sona_translation_onnx::register_ort_environment_provider(desktop_shared_ort_environment);
}
