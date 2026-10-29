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
    #[serde(
        default,
        alias = "admin_user_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub admin_user_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmbyServerInfo {
    pub id: String,
    pub server_name: String,
    pub version: String,
    pub has_subtitle_manager_plugin: bool,
    #[serde(
        default,
        alias = "admin_user_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub admin_user_id: Option<String>,
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
    #[serde(alias = "Index")]
    pub index: i32,
    #[serde(alias = "Type")]
    pub stream_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "Language")]
    pub language: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "DisplayTitle"
    )]
    pub display_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "Codec")]
    pub codec: Option<String>,
    #[serde(default, alias = "IsExternal")]
    pub is_external: bool,
    #[serde(default, alias = "IsForced")]
    pub is_forced: bool,
    #[serde(default, alias = "IsDefault")]
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

impl From<std::io::Error> for EmbyError {
    fn from(err: std::io::Error) -> Self {
        EmbyError::Io(err.to_string())
    }
}

impl From<reqwest::Error> for EmbyError {
    fn from(err: reqwest::Error) -> Self {
        EmbyError::Network(err.to_string())
    }
}

impl From<serde_json::Error> for EmbyError {
    fn from(err: serde_json::Error) -> Self {
        EmbyError::ServerError(err.to_string())
    }
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

/// Checks if a language code represents Chinese.
fn is_chinese_code(clean: &str) -> bool {
    clean == "zh"
        || clean == "chi"
        || clean == "zho"
        || clean == "cmn"
        || clean == "chs"
        || clean == "cht"
        || clean == "yue"
        || clean == "chinese"
        || clean.starts_with("zh-")
        || clean.starts_with("zh_")
        || clean.starts_with("yue-")
        || clean.starts_with("yue_")
}

/// Checks if a language code explicitly represents a known non-Chinese language.
fn is_explicit_non_chinese_code(clean: &str) -> bool {
    clean == "ja"
        || clean == "jpn"
        || clean == "japanese"
        || clean.starts_with("ja-")
        || clean.starts_with("ja_")
        || clean == "en"
        || clean == "eng"
        || clean == "english"
        || clean.starts_with("en-")
        || clean.starts_with("en_")
        || clean == "ko"
        || clean == "kor"
        || clean == "korean"
        || clean.starts_with("ko-")
        || clean.starts_with("ko_")
        || clean == "fr"
        || clean == "fra"
        || clean == "fre"
        || clean == "french"
        || clean.starts_with("fr-")
        || clean.starts_with("fr_")
        || clean == "de"
        || clean == "deu"
        || clean == "ger"
        || clean == "german"
        || clean.starts_with("de-")
        || clean.starts_with("de_")
        || clean == "es"
        || clean == "spa"
        || clean == "spanish"
        || clean.starts_with("es-")
        || clean.starts_with("es_")
        || clean == "ru"
        || clean == "rus"
        || clean == "russian"
        || clean.starts_with("ru-")
        || clean.starts_with("ru_")
}

/// Detects if language code or title corresponds to Chinese.
pub fn is_chinese_language(lang: Option<&str>, display_title: Option<&str>) -> bool {
    if let Some(l) = lang {
        let clean = l.trim().to_ascii_lowercase();
        if is_chinese_code(&clean) {
            return true;
        }
        if is_explicit_non_chinese_code(&clean) {
            return false;
        }
    }
    if let Some(t) = display_title {
        let clean = t.trim().to_ascii_lowercase();
        let chinese_phrases = [
            "中文",
            "中字",
            "简中",
            "繁中",
            "简体",
            "繁体",
            "普通话",
            "国语",
            "粤语",
            "汉语",
            "chinese",
        ];
        if chinese_phrases.iter().any(|phrase| clean.contains(phrase)) {
            return true;
        }
    }
    false
}

/// Matches preferred language with audio stream language.
pub fn language_matches(stream_lang: Option<&str>, preferred: &str) -> bool {
    let Some(sl) = stream_lang else {
        return false;
    };
    let s = sl.trim().to_ascii_lowercase();
    let p = preferred.trim().to_ascii_lowercase();

    if s == p {
        return true;
    }

    match p.as_str() {
        "zh" | "zh-cn" | "zh-tw" | "zh-hans" | "zh-hant" | "zh_cn" | "zh_tw" | "zh_hans"
        | "zh_hant" | "zh-chs" | "zh-cht" | "zh-yue" | "chinese" | "chi" | "zho" | "cmn"
        | "chs" | "cht" | "yue" => {
            s == "zh"
                || s == "chi"
                || s == "zho"
                || s == "cmn"
                || s == "chs"
                || s == "cht"
                || s == "yue"
                || s == "chinese"
                || s.starts_with("zh-")
                || s.starts_with("zh_")
                || s.starts_with("yue-")
                || s.starts_with("yue_")
        }
        "ja" | "jpn" | "japanese" => {
            s == "ja"
                || s == "jpn"
                || s == "japanese"
                || s.starts_with("ja-")
                || s.starts_with("ja_")
        }
        "en" | "eng" | "english" => {
            s == "en"
                || s == "eng"
                || s == "english"
                || s.starts_with("en-")
                || s.starts_with("en_")
        }
        "ko" | "kor" | "korean" => {
            s == "ko" || s == "kor" || s == "korean" || s.starts_with("ko-") || s.starts_with("ko_")
        }
        "fr" | "fra" | "fre" | "french" => {
            s == "fr"
                || s == "fra"
                || s == "fre"
                || s == "french"
                || s.starts_with("fr-")
                || s.starts_with("fr_")
        }
        "de" | "deu" | "ger" | "german" => {
            s == "de"
                || s == "deu"
                || s == "ger"
                || s == "german"
                || s.starts_with("de-")
                || s.starts_with("de_")
        }
        "es" | "spa" | "spanish" => {
            s == "es"
                || s == "spa"
                || s == "spanish"
                || s.starts_with("es-")
                || s.starts_with("es_")
        }
        "ru" | "rus" | "russian" => {
            s == "ru"
                || s == "rus"
                || s == "russian"
                || s.starts_with("ru-")
                || s.starts_with("ru_")
        }
        _ => {
            if p.starts_with("zh-") || p.starts_with("zh_") {
                s == "zh"
                    || s == "chi"
                    || s == "zho"
                    || s == "cmn"
                    || s == "chinese"
                    || s.starts_with("zh-")
                    || s.starts_with("zh_")
            } else if p.starts_with("ja-") || p.starts_with("ja_") {
                s == "ja"
                    || s == "jpn"
                    || s == "japanese"
                    || s.starts_with("ja-")
                    || s.starts_with("ja_")
            } else if p.starts_with("en-") || p.starts_with("en_") {
                s == "en"
                    || s == "eng"
                    || s == "english"
                    || s.starts_with("en-")
                    || s.starts_with("en_")
            } else if p.starts_with("ko-") || p.starts_with("ko_") {
                s == "ko"
                    || s == "kor"
                    || s == "korean"
                    || s.starts_with("ko-")
                    || s.starts_with("ko_")
            } else if p.starts_with("fr-") || p.starts_with("fr_") {
                s == "fr"
                    || s == "fra"
                    || s == "fre"
                    || s == "french"
                    || s.starts_with("fr-")
                    || s.starts_with("fr_")
            } else if p.starts_with("de-") || p.starts_with("de_") {
                s == "de"
                    || s == "deu"
                    || s == "ger"
                    || s == "german"
                    || s.starts_with("de-")
                    || s.starts_with("de_")
            } else if p.starts_with("es-") || p.starts_with("es_") {
                s == "es"
                    || s == "spa"
                    || s == "spanish"
                    || s.starts_with("es-")
                    || s.starts_with("es_")
            } else if p.starts_with("ru-") || p.starts_with("ru_") {
                s == "ru"
                    || s == "rus"
                    || s == "russian"
                    || s.starts_with("ru-")
                    || s.starts_with("ru_")
            } else {
                s == p
            }
        }
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
        assert!(is_chinese_language(Some("zh_CN"), None));
        assert!(is_chinese_language(None, Some("繁體中文")));
        assert!(is_chinese_language(None, Some("粤语中字")));
        assert!(is_chinese_language(Some("chs"), None));
        assert!(is_chinese_language(Some("cht"), None));
        assert!(is_chinese_language(Some("yue"), None));
        assert!(is_chinese_language(Some("yue-HK"), None));
        assert!(is_chinese_language(Some("yue_CN"), None));
        assert!(!is_chinese_language(Some("eng"), Some("English")));
        assert!(!is_chinese_language(Some("jpn"), Some("Japanese")));
        assert!(!is_chinese_language(
            Some("jpn"),
            Some("日本語（作中用語解説）")
        ));
        assert!(!is_chinese_language(Some("ja"), Some("劇中歌")));
        assert!(!is_chinese_language(None, Some("日本語（作中用語解説）")));
        assert!(!is_chinese_language(None, Some("Children of Men")));
        assert!(!is_chinese_language(None, Some("The Machine")));
        assert!(!is_chinese_language(None, Some("Architects")));
    }

    #[test]
    fn test_language_matches() {
        assert!(language_matches(Some("en-US"), "en"));
        assert!(language_matches(Some("eng"), "en"));
        assert!(language_matches(Some("ja-JP"), "ja"));
        assert!(language_matches(Some("jpn"), "ja"));
        assert!(language_matches(Some("zh-Hans"), "zh-CN"));
        assert!(language_matches(Some("zh_CN"), "zh"));
        assert!(language_matches(Some("chi"), "zh-Hans"));
        assert!(!language_matches(Some("ja"), "en"));
        assert!(!language_matches(Some("eng"), "ja"));
        assert!(language_matches(Some("chs"), "zh"));
        assert!(language_matches(Some("cht"), "zh-TW"));
        assert!(language_matches(Some("yue"), "zh-CN"));
        assert!(language_matches(Some("yue-HK"), "zh"));
        assert!(language_matches(Some("zh-CN"), "yue"));
    }

    #[test]
    fn test_emby_stream_info_serde_aliases() {
        let json = r#"{
            "Index": 1,
            "Type": "Audio",
            "Language": "eng",
            "DisplayTitle": "English",
            "Codec": "aac",
            "IsExternal": true,
            "IsForced": false,
            "IsDefault": true
        }"#;
        let stream: EmbyStreamInfo = serde_json::from_str(json).unwrap();
        assert_eq!(stream.index, 1);
        assert_eq!(stream.stream_type, "Audio");
        assert_eq!(stream.language.as_deref(), Some("eng"));
        assert_eq!(stream.display_title.as_deref(), Some("English"));
        assert_eq!(stream.codec.as_deref(), Some("aac"));
        assert!(stream.is_external);
        assert!(!stream.is_forced);
        assert!(stream.is_default);
    }

    #[test]
    fn test_emby_error_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let emby_io: EmbyError = io_err.into();
        assert!(matches!(emby_io, EmbyError::Io(_)));

        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let emby_json: EmbyError = json_err.into();
        assert!(matches!(emby_json, EmbyError::ServerError(_)));
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
