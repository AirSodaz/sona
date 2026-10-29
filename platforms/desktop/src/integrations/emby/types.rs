use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyConnectionConfig {
    #[serde(alias = "server_url")]
    pub server_url: String,
    #[serde(alias = "api_key")]
    pub api_key: String,
    #[serde(default, alias = "allow_invalid_certs")]
    pub allow_invalid_certs: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyServerInfo {
    pub id: String,
    pub server_name: String,
    pub version: String,
    pub has_subtitle_manager_plugin: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyLibrary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyStreamInfo {
    pub index: i32,
    pub stream_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(default)]
    pub is_external: bool,
    #[serde(default)]
    pub is_forced: bool,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyMediaItem {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_ticks: Option<i64>,
    #[serde(default)]
    pub media_streams: Vec<EmbyStreamInfo>,
    #[serde(default)]
    pub has_chinese_subtitles: bool,
    #[serde(default)]
    pub has_embedded_chinese: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyItemListResult {
    pub items: Vec<EmbyMediaItem>,
    pub total_record_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleUploadPayload {
    pub item_id: String,
    pub language: String,
    pub format: String,
    #[serde(default)]
    pub is_forced: bool,
    pub content: String,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum EmbyError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Unauthorized: Invalid API key or insufficient permissions")]
    Unauthorized,
    #[error("Emby.SubtitleManager plugin is not installed on the server")]
    PluginMissing,
    #[error("Duplicate subtitle already exists")]
    DuplicateSubtitle,
    #[error("Emby server error: {0}")]
    ServerError(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Normalizes server URL by trimming whitespace, ensuring scheme, and removing trailing slashes.
pub fn normalize_server_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower == "http:" || lower == "https:" || lower == "http://" || lower == "https://" {
        return String::new();
    }

    let with_scheme = if lower.starts_with("http://") || lower.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };

    let without_trailing = with_scheme.trim_end_matches('/');
    if without_trailing.eq_ignore_ascii_case("http:")
        || without_trailing.eq_ignore_ascii_case("https:")
    {
        return String::new();
    }
    without_trailing.to_string()
}

/// Detects if language code or title corresponds to Chinese.
pub fn is_chinese_language(lang: Option<&str>, display_title: Option<&str>) -> bool {
    if let Some(l) = lang {
        let clean = l.trim().to_ascii_lowercase();
        if clean == "zh"
            || clean == "chi"
            || clean == "zho"
            || clean.starts_with("zh-")
            || clean == "chinese"
            || clean == "cmn"
        {
            return true;
        }
    }
    if let Some(t) = display_title {
        let clean = t.trim().to_ascii_lowercase();
        if clean.contains('中')
            || clean.contains('简')
            || clean.contains('繁')
            || clean.contains('汉')
            || clean.contains("国语")
            || clean.contains("粤语")
            || clean.contains("chinese")
        {
            return true;
        }
    }
    false
}
/// Matches preferred language with audio stream language.
fn language_matches(stream_lang: Option<&str>, preferred: &str) -> bool {
    let Some(sl) = stream_lang else {
        return false;
    };
    let s = sl.trim().to_ascii_lowercase();
    let p = preferred.trim().to_ascii_lowercase();

    if s == p {
        return true;
    }

    match p.as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" | "chi" | "zho" => {
            s == "zh"
                || s == "chi"
                || s == "zho"
                || s.starts_with("zh-")
                || s == "chinese"
                || s == "cmn"
        }
        "ja" | "jpn" | "japanese" => s == "ja" || s == "jpn" || s == "japanese",
        "en" | "eng" | "english" => s == "en" || s == "eng" || s == "english",
        "ko" | "kor" | "korean" => s == "ko" || s == "kor" || s == "korean",
        "fr" | "fra" | "fre" | "french" => s == "fr" || s == "fra" || s == "fre" || s == "french",
        "de" | "deu" | "ger" | "german" => s == "de" || s == "deu" || s == "ger" || s == "german",
        "es" | "spa" | "spanish" => s == "es" || s == "spa" || s == "spanish",
        "ru" | "rus" | "russian" => s == "ru" || s == "rus" || s == "russian",
        _ => s == p,
    }
}

/// Selects the best audio stream matching the preferred language, falling back to default or first audio stream.
pub fn select_best_audio_stream<'a>(
    streams: &'a [EmbyStreamInfo],
    preferred_language: Option<&str>,
) -> Option<&'a EmbyStreamInfo> {
    let audio_streams: Vec<&EmbyStreamInfo> = streams
        .iter()
        .filter(|s| s.stream_type.eq_ignore_ascii_case("Audio"))
        .collect();

    if audio_streams.is_empty() {
        return None;
    }

    if let Some(pref) = preferred_language {
        let pref = pref.trim();
        if !pref.is_empty() && !pref.eq_ignore_ascii_case("auto") {
            // Find matches for preferred language
            let matches: Vec<&EmbyStreamInfo> = audio_streams
                .iter()
                .copied()
                .filter(|s| language_matches(s.language.as_deref(), pref))
                .collect();

            if let Some(stream) = matches.iter().find(|s| s.is_default).copied() {
                return Some(stream);
            }
            if let Some(stream) = matches.first().copied() {
                return Some(stream);
            }
        }
    }

    // Fallback: default audio stream, or first audio stream
    audio_streams
        .iter()
        .find(|s| s.is_default)
        .copied()
        .or_else(|| audio_streams.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_server_url() {
        assert_eq!(
            normalize_server_url("http://localhost:8096/"),
            "http://localhost:8096"
        );
        assert_eq!(
            normalize_server_url("http://localhost:8096/emby/"),
            "http://localhost:8096/emby"
        );
        assert_eq!(
            normalize_server_url("http://localhost:8096"),
            "http://localhost:8096"
        );
        assert_eq!(
            normalize_server_url("https://my-emby.server.com:8920///"),
            "https://my-emby.server.com:8920"
        );
        assert_eq!(
            normalize_server_url("192.168.1.50:8096/emby"),
            "http://192.168.1.50:8096/emby"
        );
        assert_eq!(normalize_server_url(""), "");
        assert_eq!(
            normalize_server_url("HTTP://localhost:8096/"),
            "HTTP://localhost:8096"
        );
        assert_eq!(
            normalize_server_url("HTTPS://localhost:8920/"),
            "HTTPS://localhost:8920"
        );
        assert_eq!(normalize_server_url("http:"), "");
        assert_eq!(normalize_server_url("https:"), "");
        assert_eq!(normalize_server_url("http://"), "");
        assert_eq!(normalize_server_url("https://"), "");
    }

    #[test]
    fn test_is_chinese_language() {
        assert!(is_chinese_language(Some("chi"), None));
        assert!(is_chinese_language(Some("zho"), None));
        assert!(is_chinese_language(Some("zh-CN"), None));
        assert!(is_chinese_language(Some("zh-TW"), None));
        assert!(is_chinese_language(Some("zh-Hans"), None));
        assert!(is_chinese_language(None, Some("官方中字")));
        assert!(is_chinese_language(None, Some("简体中文")));
        assert!(!is_chinese_language(Some("eng"), Some("English")));
        assert!(!is_chinese_language(Some("jpn"), Some("Japanese")));
        assert!(!is_chinese_language(None, Some("Children of Men")));
        assert!(!is_chinese_language(None, Some("The Machine")));
        assert!(!is_chinese_language(None, Some("Architects")));
    }
    #[test]
    fn test_select_best_audio_stream() {
        let streams = vec![
            EmbyStreamInfo {
                index: 0,
                stream_type: "Video".into(),
                language: None,
                display_title: None,
                codec: Some("h264".into()),
                is_external: false,
                is_forced: false,
                is_default: true,
            },
            EmbyStreamInfo {
                index: 1,
                stream_type: "Audio".into(),
                language: Some("eng".into()),
                display_title: Some("English Stereo".into()),
                codec: Some("aac".into()),
                is_external: false,
                is_forced: false,
                is_default: false,
            },
            EmbyStreamInfo {
                index: 2,
                stream_type: "Audio".into(),
                language: Some("jpn".into()),
                display_title: Some("Japanese 5.1".into()),
                codec: Some("aac".into()),
                is_external: false,
                is_forced: false,
                is_default: true,
            },
            EmbyStreamInfo {
                index: 3,
                stream_type: "Audio".into(),
                language: Some("chi".into()),
                display_title: Some("Chinese Mandarin".into()),
                codec: Some("ac3".into()),
                is_external: false,
                is_forced: false,
                is_default: false,
            },
        ];

        // Preference: Japanese
        let best_ja = select_best_audio_stream(&streams, Some("ja"));
        assert_eq!(best_ja.unwrap().index, 2);

        // Preference: English
        let best_en = select_best_audio_stream(&streams, Some("en"));
        assert_eq!(best_en.unwrap().index, 1);

        // Preference: Chinese
        let best_zh = select_best_audio_stream(&streams, Some("zh"));
        assert_eq!(best_zh.unwrap().index, 3);

        // Preference: None or auto -> falls back to default audio (jpn, index 2)
        let best_none = select_best_audio_stream(&streams, None);
        assert_eq!(best_none.unwrap().index, 2);
    }
}
