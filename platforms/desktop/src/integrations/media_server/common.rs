use futures_util::StreamExt;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use super::types::{
    MediaServerError, MediaServerLoginResult, MediaServerMediaItem, MediaServerStreamInfo,
};

/// Normalizes server URL by trimming whitespace, ensuring scheme, and removing trailing slashes.
pub fn normalize_server_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let with_scheme = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        format!("http://{}", trimmed)
    } else {
        trimmed.to_string()
    };

    let without_trailing = with_scheme.trim_end_matches('/');

    match reqwest::Url::parse(without_trailing) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("");
            let port_part = match parsed.port() {
                Some(p) => format!(":{}", p),
                None => String::new(),
            };
            let path = parsed.path().trim_end_matches('/');
            format!("{}://{}{}{}", scheme, host, port_part, path)
        }
        Err(_) => without_trailing.to_string(),
    }
}

pub struct PartFileGuard<'a> {
    path: &'a Path,
    disarmed: bool,
}

impl<'a> PartFileGuard<'a> {
    pub fn new(path: &'a Path) -> Self {
        Self {
            path,
            disarmed: false,
        }
    }

    pub fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl<'a> Drop for PartFileGuard<'a> {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

pub fn validate_item_id(item_id: &str) -> Result<&str, MediaServerError> {
    let trimmed = item_id.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(MediaServerError::InvalidConfig(format!(
            "Invalid item ID: '{item_id}'"
        )));
    }
    Ok(trimmed)
}

fn extract_base_lang(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

pub fn is_chinese_code(clean: &str) -> bool {
    matches!(
        clean,
        "zh" | "chi"
            | "zho"
            | "chinese"
            | "mandarin"
            | "cmn"
            | "cantonese"
            | "yue"
            | "wuu"
            | "hakka"
            | "hak"
            | "min"
            | "nan"
    )
}

fn language_family(clean: &str) -> &str {
    match clean {
        "en" | "eng" | "english" => "en",
        "ja" | "jpn" | "japanese" => "ja",
        "ko" | "kor" | "korean" => "ko",
        "es" | "spa" | "spanish" => "es",
        "fr" | "fra" | "fre" | "french" => "fr",
        "de" | "deu" | "ger" | "german" => "de",
        "ru" | "rus" | "russian" => "ru",
        "it" | "ita" | "italian" => "it",
        "pt" | "por" | "portuguese" => "pt",
        "yue" | "cantonese" => "yue",
        _ => clean,
    }
}

fn is_explicit_non_chinese_code(clean: &str) -> bool {
    let base = extract_base_lang(clean);
    let fam = language_family(base);
    matches!(
        fam,
        "en" | "ja" | "ko" | "es" | "fr" | "de" | "ru" | "it" | "pt"
    )
}

pub fn is_chinese_language(lang: Option<&str>, display_title: Option<&str>) -> bool {
    if let Some(l) = lang {
        let clean = l.trim().to_ascii_lowercase();
        if is_chinese_code(&clean) {
            return true;
        }
        let base = extract_base_lang(&clean);
        if is_chinese_code(base) {
            return true;
        }
    }

    if let Some(title) = display_title {
        let title_lower = title.to_ascii_lowercase();
        let lang_code = lang.map(|l| l.trim().to_ascii_lowercase());
        let lang_is_explicitly_non_chinese = lang_code
            .as_deref()
            .map(is_explicit_non_chinese_code)
            .unwrap_or(false);

        let explicit_chinese_keywords = [
            "中文字幕",
            "中文",
            "简体",
            "繁体",
            "国语",
            "普通话",
            "粤语",
            "chinese",
            "chs",
            "cht",
            "mandarin",
            "cantonese",
        ];

        let has_explicit_chinese_keyword = explicit_chinese_keywords
            .iter()
            .any(|kw| title_lower.contains(kw));

        if has_explicit_chinese_keyword {
            return true;
        }

        if !lang_is_explicitly_non_chinese {
            let broad_chinese_keywords = ["简", "繁", "双语", "汉"];
            if broad_chinese_keywords
                .iter()
                .any(|kw| title_lower.contains(kw))
            {
                return true;
            }
        }
    }

    false
}

pub fn language_matches(stream_lang: Option<&str>, preferred: &str) -> bool {
    let s_lang = match stream_lang {
        Some(l) if !l.trim().is_empty() => l.trim().to_ascii_lowercase(),
        _ => return false,
    };
    let pref = preferred.trim().to_ascii_lowercase();

    if s_lang == pref {
        return true;
    }

    let s_base = extract_base_lang(&s_lang);
    let pref_base = extract_base_lang(&pref);
    if s_base == pref_base {
        return true;
    }

    if is_chinese_code(&s_lang) && is_chinese_code(&pref) {
        return true;
    }

    language_family(s_base) == language_family(pref_base)
}

pub fn select_best_audio_stream<'a>(
    streams: &'a [MediaServerStreamInfo],
    preferred_language: Option<&str>,
) -> Option<&'a MediaServerStreamInfo> {
    let audio_streams: Vec<&'a MediaServerStreamInfo> = streams
        .iter()
        .filter(|s| s.stream_type.eq_ignore_ascii_case("Audio"))
        .collect();

    if audio_streams.is_empty() {
        return None;
    }

    if let Some(pref) = preferred_language {
        if let Some(s) = audio_streams
            .iter()
            .find(|s| language_matches(s.language.as_deref(), pref) && s.is_default)
        {
            return Some(*s);
        }

        if let Some(s) = audio_streams
            .iter()
            .find(|s| language_matches(s.language.as_deref(), pref))
        {
            return Some(*s);
        }
    }

    if let Some(s) = audio_streams.iter().find(|s| s.is_default) {
        return Some(*s);
    }

    audio_streams.first().copied()
}

pub fn parse_media_streams(streams_val: &Value) -> (Vec<MediaServerStreamInfo>, bool, bool) {
    let mut media_streams = Vec::new();
    let mut has_chinese_subtitles = false;
    let mut has_embedded_chinese = false;

    if let Some(streams) = streams_val.as_array() {
        for s in streams {
            let stream_type = s
                .get("Type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let index = s.get("Index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let language = s
                .get("Language")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            let display_title = s
                .get("DisplayTitle")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            let codec = s
                .get("Codec")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            let is_external = s
                .get("IsExternal")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_forced = s.get("IsForced").and_then(|v| v.as_bool()).unwrap_or(false);
            let is_default = s
                .get("IsDefault")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if stream_type.eq_ignore_ascii_case("Subtitle") {
                let is_zh = is_chinese_language(language.as_deref(), display_title.as_deref());
                if is_zh {
                    has_chinese_subtitles = true;
                    if !is_external {
                        has_embedded_chinese = true;
                    }
                }
            }

            media_streams.push(MediaServerStreamInfo {
                index,
                stream_type,
                language,
                display_title,
                codec,
                is_external,
                is_forced,
                is_default,
            });
        }
    }

    (media_streams, has_chinese_subtitles, has_embedded_chinese)
}

pub fn parse_media_items(raw_items: &[Value]) -> Vec<MediaServerMediaItem> {
    let mut items = Vec::new();
    for raw in raw_items {
        let id = raw
            .get("Id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let name = raw
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let series_name = raw
            .get("SeriesName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let season_name = raw
            .get("SeasonName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let season_index = raw
            .get("ParentIndexNumber")
            .and_then(|v| v.as_i64())
            .map(|i| i as i32);
        let episode_index = raw
            .get("IndexNumber")
            .and_then(|v| v.as_i64())
            .map(|i| i as i32);
        let runtime_ticks = raw.get("RunTimeTicks").and_then(|v| v.as_i64());
        let item_type = raw
            .get("Type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let media_source_id = raw
            .get("MediaSources")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.get("Id").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .or_else(|| {
                if !id.is_empty() {
                    Some(format!("mediasource_{id}"))
                } else {
                    None
                }
            });
        let container = raw
            .get("MediaSources")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.get("Container").and_then(|v| v.as_str()))
            .or_else(|| raw.get("Container").and_then(|v| v.as_str()))
            .map(|s| s.to_string());

        let (media_streams, has_chinese_subtitles, has_embedded_chinese) =
            if let Some(streams_val) = raw.get("MediaStreams") {
                parse_media_streams(streams_val)
            } else {
                (Vec::new(), false, false)
            };

        items.push(MediaServerMediaItem {
            id,
            name,
            series_name,
            season_name,
            season_index,
            episode_index,
            runtime_ticks,
            media_streams,
            has_chinese_subtitles,
            has_embedded_chinese,
            item_type,
            media_source_id,
            container,
        });
    }
    items
}

pub fn parse_admin_user_id(users_json: &Value) -> Option<String> {
    let users = users_json
        .as_array()
        .or_else(|| users_json.get("Items").and_then(|v| v.as_array()))?;
    for user in users {
        let policy = user.get("Policy").or_else(|| user.get("policy"));
        let is_admin = policy
            .and_then(|p| {
                p.get("IsAdministrator")
                    .or_else(|| p.get("isAdministrator"))
            })
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let is_disabled = policy
            .and_then(|p| p.get("IsDisabled").or_else(|| p.get("isDisabled")))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_admin && !is_disabled {
            let id_val = user.get("Id").or_else(|| user.get("id"));
            if let Some(id) = id_val.and_then(|v| v.as_str()) {
                let trimmed = id.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

pub fn parse_login_response(
    json: &Value,
    fallback_user: &str,
) -> Result<MediaServerLoginResult, MediaServerError> {
    let access_token = json
        .get("AccessToken")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            MediaServerError::ServerError("Missing AccessToken in login response".into())
        })?
        .to_string();

    let user_obj = json.get("User").and_then(|v| v.as_object());
    let user_id = user_obj
        .and_then(|u| u.get("Id").or_else(|| u.get("id")))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let user_name = user_obj
        .and_then(|u| u.get("Name").or_else(|| u.get("name")))
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_user)
        .to_string();

    let is_admin = user_obj
        .and_then(|u| u.get("Policy").or_else(|| u.get("policy")))
        .and_then(|p| {
            p.get("IsAdministrator")
                .or_else(|| p.get("isAdministrator"))
        })
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(MediaServerLoginResult {
        access_token,
        user_id,
        user_name,
        is_admin,
    })
}
pub fn check_plugin_response_body_helper(body_text: &str) -> Result<(), MediaServerError> {
    if let Ok(json) = serde_json::from_str::<Value>(body_text) {
        let is_success = json
            .get("Success")
            .or_else(|| json.get("success"))
            .and_then(|v| v.as_bool());
        if is_success == Some(false) {
            let msg = json
                .get("Message")
                .or_else(|| json.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Operation failed");
            let lower = msg.to_ascii_lowercase();
            if lower.contains("duplicate")
                || lower.contains("already exist")
                || msg.contains("已存在")
                || msg.contains("重复")
                || msg.contains("既に存在")
                || msg.contains("重複")
                || msg.contains("이미 있습니다")
                || msg.contains("중복")
            {
                return Err(MediaServerError::DuplicateSubtitle);
            } else {
                return Err(MediaServerError::ServerError(msg.to_string()));
            }
        }
    }
    Ok(())
}

pub async fn download_audio_stream_common(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    item_id: &str,
    stream_index: Option<i32>,
    destination_path: &Path,
) -> Result<u64, MediaServerError> {
    let clean_id = validate_item_id(item_id)?;
    let clean_base = base_url.trim_end_matches('/');
    let url = format!("{clean_base}/Audio/{clean_id}/stream.mp3");

    if let Some(parent) = destination_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| MediaServerError::Io(e.to_string()))?;
    }

    let mut last_err = None;
    for attempt in 0..3 {
        let part_path = destination_path.with_extension(format!("part_{}", uuid::Uuid::new_v4()));
        let mut part_guard = PartFileGuard::new(&part_path);
        let mut req = client
            .get(&url)
            .timeout(Duration::from_secs(600))
            .header("X-Emby-Token", api_key)
            .query(&[("api_key", api_key)]);

        let idx_str;
        if let Some(idx) = stream_index {
            idx_str = idx.to_string();
            req = req.query(&[("AudioStreamIndex", &idx_str)]);
        }

        match req.send().await {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED
                    || resp.status() == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(MediaServerError::Unauthorized);
                }
                if resp.status().is_server_error() {
                    last_err = Some(MediaServerError::ServerError(format!(
                        "Audio download failed with server error: HTTP {}",
                        resp.status()
                    )));
                    tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
                    continue;
                }
                if !resp.status().is_success() {
                    return Err(MediaServerError::ServerError(format!(
                        "Audio download failed: HTTP {}",
                        resp.status()
                    )));
                }

                let file = match tokio::fs::File::create(&part_path).await {
                    Ok(f) => f,
                    Err(e) => {
                        return Err(MediaServerError::Io(e.to_string()));
                    }
                };
                let mut file_opt = Some(file);

                let mut stream = resp.bytes_stream();
                let mut bytes_written: u64 = 0;

                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(chunk) => {
                            let write_res = if let Some(f) = file_opt.as_mut() {
                                f.write_all(&chunk).await
                            } else {
                                Ok(())
                            };
                            if let Err(e) = write_res {
                                drop(file_opt.take());
                                return Err(MediaServerError::Io(e.to_string()));
                            }
                            bytes_written += chunk.len() as u64;
                        }
                        Err(e) => {
                            drop(file_opt.take());
                            last_err = Some(MediaServerError::Network(format!(
                                "Stream interrupted on attempt {attempt}: {e}"
                            )));
                            break;
                        }
                    }
                }

                if let Some(mut file) = file_opt {
                    if let Err(e) = file.flush().await {
                        drop(file);
                        return Err(MediaServerError::Io(e.to_string()));
                    }
                    drop(file);

                    if bytes_written == 0 {
                        last_err = Some(MediaServerError::ServerError(
                            "Downloaded audio stream is empty (0 bytes)".into(),
                        ));
                    } else if let Err(e) = tokio::fs::rename(&part_path, destination_path).await {
                        return Err(MediaServerError::Io(e.to_string()));
                    } else {
                        part_guard.disarm();
                        return Ok(bytes_written);
                    }
                }
            }
            Err(e) => {
                last_err = Some(MediaServerError::Network(format!(
                    "Download request failed on attempt {attempt}: {e}"
                )));
            }
        }

        tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
    }

    Err(last_err
        .unwrap_or_else(|| MediaServerError::Network("Download audio stream failed".into())))
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
            normalize_server_url("localhost:8096"),
            "http://localhost:8096"
        );
        assert_eq!(
            normalize_server_url("https://emby.example.com/emby/"),
            "https://emby.example.com/emby"
        );
    }

    #[test]
    fn test_is_chinese_language() {
        assert!(is_chinese_language(Some("zh"), None));
        assert!(is_chinese_language(Some("chi"), None));
        assert!(is_chinese_language(Some("zho"), None));
        assert!(is_chinese_language(Some("zh-CN"), None));
        assert!(is_chinese_language(None, Some("Movie.chs.srt")));
        assert!(is_chinese_language(None, Some("中文字幕")));
        assert!(!is_chinese_language(Some("en"), Some("English")));
    }
}
