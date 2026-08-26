use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

const MODEL_MIRRORS_JSON: &str = include_str!("model-mirrors.json");

/// Alternate distribution URLs for preset artifacts, keyed by model id and
/// artifact filename. Populated for models whose primary host (GitHub,
/// HuggingFace) is unreachable for some users; ModelScope serves as the
/// last-resort candidate during downloads.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelMirrorsFile {
    #[serde(default)]
    modelscope: HashMap<String, HashMap<String, String>>,
}

static MODEL_MIRRORS: OnceLock<ModelMirrorsFile> = OnceLock::new();

fn model_mirrors() -> &'static ModelMirrorsFile {
    MODEL_MIRRORS.get_or_init(|| {
        serde_json::from_str(MODEL_MIRRORS_JSON).expect("model-mirrors.json should be valid")
    })
}

/// Returns the ModelScope alternate URL for a preset artifact, if curated.
pub fn modelscope_mirror_url(model_id: &str, filename: &str) -> Option<&'static str> {
    model_mirrors()
        .modelscope
        .get(model_id)?
        .get(filename)
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_or_filename_yields_none() {
        assert!(modelscope_mirror_url("missing-model", "model.onnx").is_none());
        assert!(
            modelscope_mirror_url(
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
                "not-curated.tar.bz2"
            )
            .is_none()
        );
    }
}
