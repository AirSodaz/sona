//! Download-mirror resolution for model artifacts.
//!
//! Mirrors are source-host aware: a GitHub proxy only rewrites GitHub URLs and
//! `hf-mirror.com` only rewrites HuggingFace URLs. [`download_candidates`]
//! builds the ordered URL chain attempted by the download loops: direct first,
//! then the configured mirror (when applicable), then a curated ModelScope
//! alternate distribution as the last resort.

/// User-selectable download mirror strategy (persisted as
/// `modelDownloadMirror`: `auto` | `direct` | `ghproxy` | `ghnet` |
/// `hf-mirror`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadMirror {
    Auto,
    Direct,
    GhProxy,
    GhNet,
    HfMirror,
}

/// Host family a download URL belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadSource {
    GitHub,
    HuggingFace,
    ModelScope,
    Other,
}

const GHPROXY_PREFIX: &str = "https://mirror.ghproxy.com/";
const GHNET_PREFIX: &str = "https://ghproxy.net/";
const HF_MIRROR_ORIGIN: &str = "https://hf-mirror.com";
const HUGGINGFACE_ORIGIN: &str = "https://huggingface.co";

/// Parses a persisted mirror key; unknown or empty values fall back to `Auto`.
pub fn parse_download_mirror(key: &str) -> DownloadMirror {
    match key {
        "direct" => DownloadMirror::Direct,
        "ghproxy" => DownloadMirror::GhProxy,
        "ghnet" => DownloadMirror::GhNet,
        "hf-mirror" => DownloadMirror::HfMirror,
        _ => DownloadMirror::Auto,
    }
}

fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('.')
}

/// Classifies a download URL by its host family.
pub fn detect_download_source(url: &str) -> DownloadSource {
    let host = host_of(url);
    if host == "github.com"
        || host.ends_with(".github.com")
        || host.ends_with("githubusercontent.com")
    {
        DownloadSource::GitHub
    } else if host == "huggingface.co" || host.ends_with(".huggingface.co") {
        DownloadSource::HuggingFace
    } else if host == "modelscope.cn" || host.ends_with(".modelscope.cn") {
        DownloadSource::ModelScope
    } else {
        DownloadSource::Other
    }
}

/// Rewrites `url` for the given mirror, or `None` when the mirror does not
/// serve the URL's source host.
pub fn apply_download_mirror(url: &str, mirror: DownloadMirror) -> Option<String> {
    match (mirror, detect_download_source(url)) {
        (DownloadMirror::GhProxy, DownloadSource::GitHub) => Some(format!("{GHPROXY_PREFIX}{url}")),
        (DownloadMirror::GhNet, DownloadSource::GitHub) => Some(format!("{GHNET_PREFIX}{url}")),
        (DownloadMirror::HfMirror, DownloadSource::HuggingFace) => url
            .strip_prefix(HUGGINGFACE_ORIGIN)
            .map(|path_and_query| format!("{HF_MIRROR_ORIGIN}{path_and_query}")),
        _ => None,
    }
}

/// The mirror tried automatically after a direct download fails.
fn auto_mirror_for(source: DownloadSource) -> Option<DownloadMirror> {
    match source {
        DownloadSource::GitHub => Some(DownloadMirror::GhNet),
        DownloadSource::HuggingFace => Some(DownloadMirror::HfMirror),
        DownloadSource::ModelScope | DownloadSource::Other => None,
    }
}

/// Ordered download URLs to attempt: direct first, then the configured mirror
/// (when it serves the URL's host; `Auto` picks the source-appropriate one),
/// then the curated ModelScope alternate distribution.
pub fn download_candidates(
    url: &str,
    mirror: DownloadMirror,
    alternate_url: Option<&str>,
) -> Vec<String> {
    let mut candidates = vec![url.to_string()];

    match mirror {
        DownloadMirror::Direct => {}
        DownloadMirror::Auto => {
            if let Some(auto_mirror) = auto_mirror_for(detect_download_source(url)) {
                if let Some(mirrored) = apply_download_mirror(url, auto_mirror) {
                    candidates.push(mirrored);
                }
            }
        }
        explicit => {
            if let Some(mirrored) = apply_download_mirror(url, explicit) {
                candidates.push(mirrored);
            }
        }
    }

    if let Some(alternate) = alternate_url {
        if detect_download_source(url) != DownloadSource::ModelScope
            && !candidates.iter().any(|candidate| candidate == alternate)
        {
            candidates.push(alternate.to_string());
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    const GITHUB_URL: &str =
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";
    const HUGGINGFACE_URL: &str =
        "https://huggingface.co/ggml-org/Qwen3-ASR-0.6B-GGUF/resolve/abc/model.gguf?download=true";
    const MODELSCOPE_URL: &str =
        "https://www.modelscope.cn/models/org/name/resolve/master/file.onnx";

    #[test]
    fn parse_handles_all_keys_and_defaults_to_auto() {
        assert_eq!(parse_download_mirror("auto"), DownloadMirror::Auto);
        assert_eq!(parse_download_mirror("direct"), DownloadMirror::Direct);
        assert_eq!(parse_download_mirror("ghproxy"), DownloadMirror::GhProxy);
        assert_eq!(parse_download_mirror("ghnet"), DownloadMirror::GhNet);
        assert_eq!(parse_download_mirror("hf-mirror"), DownloadMirror::HfMirror);
        assert_eq!(parse_download_mirror(""), DownloadMirror::Auto);
        assert_eq!(parse_download_mirror("nope"), DownloadMirror::Auto);
    }

    #[test]
    fn detect_classifies_source_hosts() {
        assert_eq!(detect_download_source(GITHUB_URL), DownloadSource::GitHub);
        assert_eq!(
            detect_download_source("https://raw.githubusercontent.com/org/repo/main/file"),
            DownloadSource::GitHub
        );
        assert_eq!(
            detect_download_source(HUGGINGFACE_URL),
            DownloadSource::HuggingFace
        );
        assert_eq!(
            detect_download_source(MODELSCOPE_URL),
            DownloadSource::ModelScope
        );
        assert_eq!(
            detect_download_source("https://example.com/model.tar.bz2"),
            DownloadSource::Other
        );
    }

    #[test]
    fn apply_rewrites_only_supported_hosts() {
        assert_eq!(
            apply_download_mirror(GITHUB_URL, DownloadMirror::GhProxy).as_deref(),
            Some(
                "https://mirror.ghproxy.com/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx"
            ),
        );
        assert_eq!(
            apply_download_mirror(HUGGINGFACE_URL, DownloadMirror::HfMirror).as_deref(),
            Some(
                "https://hf-mirror.com/ggml-org/Qwen3-ASR-0.6B-GGUF/resolve/abc/model.gguf?download=true"
            ),
        );
        assert_eq!(
            apply_download_mirror(HUGGINGFACE_URL, DownloadMirror::GhProxy),
            None
        );
        assert_eq!(
            apply_download_mirror(GITHUB_URL, DownloadMirror::HfMirror),
            None
        );
        assert_eq!(
            apply_download_mirror(MODELSCOPE_URL, DownloadMirror::Auto),
            None
        );
    }

    #[test]
    fn candidates_auto_chains_direct_mirror_and_alternate() {
        assert_eq!(
            download_candidates(
                GITHUB_URL,
                DownloadMirror::Auto,
                Some("https://modelscope.cn/m")
            ),
            vec![
                GITHUB_URL.to_string(),
                format!("{GHNET_PREFIX}{GITHUB_URL}"),
                "https://modelscope.cn/m".to_string(),
            ],
        );
        assert_eq!(
            download_candidates(HUGGINGFACE_URL, DownloadMirror::Auto, None),
            vec![
                HUGGINGFACE_URL.to_string(),
                "https://hf-mirror.com/ggml-org/Qwen3-ASR-0.6B-GGUF/resolve/abc/model.gguf?download=true".to_string(),
            ],
        );
    }

    #[test]
    fn candidates_direct_keeps_only_alternate() {
        assert_eq!(
            download_candidates(
                GITHUB_URL,
                DownloadMirror::Direct,
                Some("https://modelscope.cn/m")
            ),
            vec![
                GITHUB_URL.to_string(),
                "https://modelscope.cn/m".to_string()
            ],
        );
    }

    #[test]
    fn candidates_unsupported_explicit_mirror_falls_back_to_direct() {
        assert_eq!(
            download_candidates(HUGGINGFACE_URL, DownloadMirror::GhProxy, None),
            vec![HUGGINGFACE_URL.to_string()],
        );
    }

    #[test]
    fn candidates_skip_alternate_for_modelscope_and_dedupe() {
        assert_eq!(
            download_candidates(MODELSCOPE_URL, DownloadMirror::Auto, Some(MODELSCOPE_URL)),
            vec![MODELSCOPE_URL.to_string()],
        );
        assert_eq!(
            download_candidates(GITHUB_URL, DownloadMirror::Auto, Some(GITHUB_URL)),
            vec![
                GITHUB_URL.to_string(),
                format!("{GHNET_PREFIX}{GITHUB_URL}"),
            ],
        );
    }
}
