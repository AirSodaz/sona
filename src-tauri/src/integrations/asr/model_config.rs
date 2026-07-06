pub use crate::core::model_config::ModelFileConfig;
pub use sona_local_asr::punctuation::{Punctuation, load_punctuation};
pub use sona_local_asr::recognizer::{
    ModelType, Recognizer, RecognizerCreateResult, RecognizerInner, SafeOfflineRecognizer,
    SafeOnlineRecognizer, SafeStream, build_model_config, create_recognizer_with_gpu_plan,
};
use std::path::Path;

pub struct SafeVad(pub sona_local_asr::audio::VadDetector);
unsafe impl Send for SafeVad {}
unsafe impl Sync for SafeVad {}

pub fn load_vad(vad_model: Option<String>) -> Option<SafeVad> {
    let v_path = vad_model?;

    if v_path.is_empty() {
        println!(
            "[Sherpa] load_vad: Path is empty or does not exist: {}",
            v_path
        );
        return None;
    }

    match sona_local_asr::audio::create_vad_detector(Path::new(&v_path), 60.0) {
        Ok(vad) => {
            println!("[Sherpa] load_vad: VAD successfully created!");
            Some(SafeVad(vad))
        }
        Err(error) => {
            println!("[Sherpa] load_vad: {error}");
            None
        }
    }
}
