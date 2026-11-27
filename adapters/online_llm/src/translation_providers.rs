use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sona_core::llm::provider_protocol::StandardLlmResponse;
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind, LlmTranslationRequest};
use uuid::Uuid;

use crate::transport::{LlmApiUrl, post_form_request, post_json_request};

const DEEPL_FREE_HOST: &str = "https://api-free.deepl.com";
const DEEPL_PRO_HOST: &str = "https://api.deepl.com";

fn normalized_lang_lower(raw: &str) -> String {
    raw.trim().replace('_', "-").to_ascii_lowercase()
}

fn primary_subtag(normalized: &str) -> &str {
    normalized.split('-').next().unwrap_or(normalized)
}

fn is_simplified_chinese(normalized: &str) -> bool {
    matches!(
        normalized,
        "zh" | "zh-cn" | "zh-hans" | "zh-chs" | "zh-sg"
    )
}

fn is_traditional_chinese(normalized: &str) -> bool {
    matches!(
        normalized,
        "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant" | "zh-cht"
    )
}

pub fn map_deepl_lang(raw: &str, is_target: bool) -> String {
    let norm = normalized_lang_lower(raw);
    if is_simplified_chinese(&norm) {
        return if is_target { "ZH-HANS" } else { "ZH" }.to_string();
    }
    if is_traditional_chinese(&norm) {
        return if is_target { "ZH-HANT" } else { "ZH" }.to_string();
    }
    if is_target {
        match norm.as_str() {
            "en-us" => return "EN-US".to_string(),
            "en-gb" => return "EN-GB".to_string(),
            "pt-br" => return "PT-BR".to_string(),
            "pt-pt" => return "PT-PT".to_string(),
            _ => {}
        }
    }
    primary_subtag(&norm).to_ascii_uppercase()
}

pub fn map_microsoft_lang(raw: &str) -> String {
    let norm = normalized_lang_lower(raw);
    if is_simplified_chinese(&norm) {
        return "zh-Hans".to_string();
    }
    if is_traditional_chinese(&norm) {
        return "zh-Hant".to_string();
    }
    match norm.as_str() {
        "pt-pt" => "pt-PT".to_string(),
        "pt-br" => "pt-BR".to_string(),
        "fr-ca" => "fr-CA".to_string(),
        _ => primary_subtag(&norm).to_string(),
    }
}

pub fn map_libre_translate_lang(raw: &str) -> String {
    let norm = normalized_lang_lower(raw);
    if is_simplified_chinese(&norm) {
        return "zh".to_string();
    }
    if is_traditional_chinese(&norm) {
        return "zt".to_string();
    }
    primary_subtag(&norm).to_string()
}

pub fn map_baidu_lang(raw: &str) -> String {
    let norm = normalized_lang_lower(raw);
    if is_simplified_chinese(&norm) {
        return "zh".to_string();
    }
    if is_traditional_chinese(&norm) {
        return "cht".to_string();
    }
    match primary_subtag(&norm) {
        "ja" => "jp",
        "ko" => "kor",
        "fr" => "fra",
        "es" => "spa",
        "ar" => "ara",
        "vi" => "vie",
        "da" => "dan",
        "bg" => "bul",
        "et" => "est",
        "fi" => "fin",
        "ro" => "rom",
        "sl" => "slo",
        "sv" => "swe",
        other => other,
    }
    .to_string()
}

pub fn map_youdao_lang(raw: &str) -> String {
    let norm = normalized_lang_lower(raw);
    if is_simplified_chinese(&norm) {
        return "zh-CHS".to_string();
    }
    if is_traditional_chinese(&norm) {
        return "zh-CHT".to_string();
    }
    primary_subtag(&norm).to_string()
}

pub fn resolve_deepl_base_url(base_url: &str, api_key: &str) -> String {
    let trimmed = base_url.trim();
    let base = if trimmed.is_empty() {
        DEEPL_FREE_HOST
    } else {
        trimmed
    };
    let normalized = base.trim_end_matches('/');
    let trimmed_key = api_key.trim();
    if normalized.eq_ignore_ascii_case(DEEPL_FREE_HOST)
        && !trimmed_key.is_empty()
        && !trimmed_key.ends_with(":fx")
    {
        DEEPL_PRO_HOST.to_string()
    } else {
        base.to_string()
    }
}

fn ensure_endpoint_suffix(base_url: &str, suffix: &str) -> Result<LlmApiUrl, LlmPortError> {
    let parsed = LlmApiUrl::parse(base_url)?;
    let path = parsed.reqwest_url().path().trim_end_matches('/').to_string();
    let expected = format!("/{}", suffix.trim_start_matches('/'));
    if path.ends_with(&expected) {
        Ok(parsed)
    } else {
        parsed.join(suffix)
    }
}

fn require_non_empty<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str, LlmPortError> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            LlmPortError::new(
                LlmPortErrorKind::InvalidRequest,
                format!("{field} cannot be empty"),
            )
        })
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Computes the Youdao v3 `input` string for signing:
/// if `len <= 20`, returns `q`; otherwise `first_10 + len + last_10`.
pub fn youdao_sign_input(q: &str) -> String {
    let chars: Vec<char> = q.chars().collect();
    let len = chars.len();
    if len <= 20 {
        q.to_string()
    } else {
        let head: String = chars[..10].iter().collect();
        let tail: String = chars[len - 10..].iter().collect();
        format!("{head}{len}{tail}")
    }
}

pub fn youdao_v3_sign(
    app_key: &str,
    q: &str,
    salt: &str,
    curtime: &str,
    app_secret: &str,
) -> String {
    let input = youdao_sign_input(q);
    let raw = format!("{app_key}{input}{salt}{curtime}{app_secret}");
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

/// Pure-Rust RFC 1321 MD5 digest for Baidu Translate API signing.
pub fn md5_hex(input: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut data = input.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    for chunk in data.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let offset = i * 4;
            *word = u32::from_le_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };
            let temp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = temp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    hex::encode(out)
}

#[derive(Deserialize)]
struct DeeplTranslation {
    text: String,
}

#[derive(Deserialize)]
struct DeeplResponse {
    translations: Vec<DeeplTranslation>,
}

async fn translate_deepl(
    config: &LlmConfig,
    texts: Vec<String>,
    target_language: &str,
    source_language: Option<&str>,
) -> Result<Vec<String>, LlmPortError> {
    let api_key = require_non_empty(Some(&config.api_key), "API key")?;
    let resolved_host = resolve_deepl_base_url(&config.base_url, api_key);
    let url = ensure_endpoint_suffix(&resolved_host, "v2/translate")?;

    let mut payload = serde_json::Map::new();
    payload.insert("text".to_string(), json!(texts));
    payload.insert(
        "target_lang".to_string(),
        json!(map_deepl_lang(target_language, true)),
    );
    if let Some(src) = source_language
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
    {
        payload.insert("source_lang".to_string(), json!(map_deepl_lang(src, false)));
    }

    let response = post_json_request(
        &url,
        vec![("Authorization", format!("DeepL-Auth-Key {api_key}"))],
        Value::Object(payload),
        config.timeout_seconds,
    )
    .await?;

    let parsed: DeeplResponse = serde_json::from_value(response).map_err(|e| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Invalid DeepL response: {e}"),
        )
    })?;
    Ok(parsed.translations.into_iter().map(|t| t.text).collect())
}

#[derive(Deserialize)]
struct MicrosoftTranslation {
    text: String,
}

#[derive(Deserialize)]
struct MicrosoftEntry {
    translations: Vec<MicrosoftTranslation>,
}

async fn translate_microsoft(
    config: &LlmConfig,
    texts: Vec<String>,
    target_language: &str,
    source_language: Option<&str>,
) -> Result<Vec<String>, LlmPortError> {
    let api_key = require_non_empty(Some(&config.api_key), "API key")?;
    let base = ensure_endpoint_suffix(&config.base_url, "translate")?;
    let to_lang = map_microsoft_lang(target_language);
    let mut query = format!("api-version=3.0&to={}", urlencoding::encode(&to_lang));
    if let Some(src) = source_language
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
    {
        let from_lang = map_microsoft_lang(src);
        query.push_str(&format!("&from={}", urlencoding::encode(&from_lang)));
    }
    let url = base.with_query(&query)?;

    let mut headers = vec![("Ocp-Apim-Subscription-Key", api_key.to_string())];
    if let Some(region) = config
        .api_region
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
    {
        headers.push(("Ocp-Apim-Subscription-Region", region.to_string()));
    }

    let body = Value::Array(
        texts
            .into_iter()
            .map(|text| json!({ "Text": text }))
            .collect(),
    );
    let response = post_json_request(&url, headers, body, config.timeout_seconds).await?;
    let parsed: Vec<MicrosoftEntry> = serde_json::from_value(response).map_err(|e| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Invalid Microsoft Translator response: {e}"),
        )
    })?;

    Ok(parsed
        .into_iter()
        .map(|entry| {
            entry
                .translations
                .into_iter()
                .next()
                .map(|t| t.text)
                .unwrap_or_default()
        })
        .collect())
}

async fn translate_libre_translate(
    config: &LlmConfig,
    texts: Vec<String>,
    target_language: &str,
    source_language: Option<&str>,
) -> Result<Vec<String>, LlmPortError> {
    let url = ensure_endpoint_suffix(&config.base_url, "translate")?;
    let source = source_language
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
        .map(map_libre_translate_lang)
        .unwrap_or_else(|| "auto".to_string());
    let target = map_libre_translate_lang(target_language);

    let single_item = texts.len() == 1;
    let mut payload = serde_json::Map::new();
    if single_item {
        payload.insert("q".to_string(), json!(texts[0]));
    } else {
        payload.insert("q".to_string(), json!(texts));
    }
    payload.insert("source".to_string(), json!(source));
    payload.insert("target".to_string(), json!(target));
    payload.insert("format".to_string(), json!("text"));
    let trimmed_key = config.api_key.trim();
    if !trimmed_key.is_empty() {
        payload.insert("api_key".to_string(), json!(trimmed_key));
    }

    let response = post_json_request(
        &url,
        Vec::new(),
        Value::Object(payload),
        config.timeout_seconds,
    )
    .await?;

    let translated = response.get("translatedText").ok_or_else(|| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "Invalid LibreTranslate response: missing translatedText",
        )
    })?;

    if let Some(single) = translated.as_str() {
        Ok(vec![single.to_string()])
    } else if let Some(arr) = translated.as_array() {
        arr.iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    LlmPortError::new(
                        LlmPortErrorKind::Protocol,
                        "Invalid LibreTranslate response element",
                    )
                })
            })
            .collect()
    } else {
        Err(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "Invalid LibreTranslate response format",
        ))
    }
}

#[derive(Deserialize)]
struct DeeplxResponse {
    #[serde(default)]
    code: Option<i64>,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

async fn translate_deeplx_single(
    config: &LlmConfig,
    text: &str,
    target_language: &str,
    source_language: Option<&str>,
) -> Result<String, LlmPortError> {
    let parsed_base = LlmApiUrl::parse(&config.base_url)?;
    let url = if parsed_base.reqwest_url().path().trim_matches('/').is_empty() {
        parsed_base.join("translate")?
    } else {
        parsed_base
    };

    let source_lang = source_language
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
        .map(|s| map_deepl_lang(s, false))
        .unwrap_or_else(|| "auto".to_string());
    let target_lang = map_deepl_lang(target_language, true);

    let mut headers = Vec::new();
    let trimmed_key = config.api_key.trim();
    if !trimmed_key.is_empty() {
        headers.push(("Authorization", format!("Bearer {trimmed_key}")));
    }

    let response = post_json_request(
        &url,
        headers,
        json!({
            "text": text,
            "source_lang": source_lang,
            "target_lang": target_lang,
        }),
        config.timeout_seconds,
    )
    .await?;

    let parsed: DeeplxResponse = serde_json::from_value(response).map_err(|e| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Invalid DeepLX response: {e}"),
        )
    })?;
    if let Some(code) = parsed.code
        && code != 200
    {
        let kind = match code {
            401 => LlmPortErrorKind::Authentication,
            403 => LlmPortErrorKind::Permission,
            429 => LlmPortErrorKind::RateLimited,
            _ => LlmPortErrorKind::Protocol,
        };
        return Err(LlmPortError::new(
            kind,
            parsed
                .message
                .unwrap_or_else(|| format!("DeepLX error code {code}")),
        ));
    }

    parsed.data.ok_or_else(|| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "Invalid DeepLX response: missing data",
        )
    })
}

#[derive(Deserialize)]
struct BaiduTransResult {
    dst: String,
}

#[derive(Deserialize)]
struct BaiduResponse {
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_msg: Option<String>,
    #[serde(default)]
    trans_result: Option<Vec<BaiduTransResult>>,
}

async fn translate_baidu_single(
    config: &LlmConfig,
    text: &str,
    target_language: &str,
    source_language: Option<&str>,
) -> Result<String, LlmPortError> {
    let app_id = require_non_empty(config.app_id.as_deref(), "App ID")?;
    let secret_key = require_non_empty(Some(&config.api_key), "Secret key")?;
    let url = LlmApiUrl::parse(&config.base_url)?;
    let salt = Uuid::new_v4().simple().to_string();
    let sign_input = format!("{app_id}{text}{salt}{secret_key}");
    let sign = md5_hex(sign_input.as_bytes());
    let from = source_language
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
        .map(map_baidu_lang)
        .unwrap_or_else(|| "auto".to_string());
    let to = map_baidu_lang(target_language);

    let response = post_form_request(
        &url,
        &[
            ("q", text),
            ("from", &from),
            ("to", &to),
            ("appid", app_id),
            ("salt", &salt),
            ("sign", &sign),
        ],
        config.timeout_seconds,
    )
    .await?;

    let parsed: BaiduResponse = serde_json::from_value(response).map_err(|e| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Invalid Baidu Translate response: {e}"),
        )
    })?;
    if let Some(code) = parsed.error_code
        && code != "52000"
    {
        let kind = match code.as_str() {
            "52003" | "54001" => LlmPortErrorKind::Authentication,
            "54003" | "54005" => LlmPortErrorKind::RateLimited,
            "58002" => LlmPortErrorKind::Unavailable,
            _ => LlmPortErrorKind::Protocol,
        };
        let msg = parsed.error_msg.unwrap_or_default();
        return Err(LlmPortError::new(
            kind,
            format!("Baidu Translate error {code}: {msg}"),
        ));
    }

    let results = parsed.trans_result.ok_or_else(|| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "Invalid Baidu Translate response: missing trans_result",
        )
    })?;
    Ok(results
        .into_iter()
        .map(|item| item.dst)
        .collect::<Vec<_>>()
        .join("\n"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct YoudaoResponse {
    error_code: String,
    #[serde(default)]
    translation: Option<Vec<String>>,
}

async fn translate_youdao_single(
    config: &LlmConfig,
    text: &str,
    target_language: &str,
    source_language: Option<&str>,
) -> Result<String, LlmPortError> {
    let app_key = require_non_empty(config.app_id.as_deref(), "App Key")?;
    let app_secret = require_non_empty(Some(&config.api_key), "App Secret")?;
    let url = LlmApiUrl::parse(&config.base_url)?;
    let salt = Uuid::new_v4().to_string();
    let curtime = current_unix_seconds().to_string();
    let sign = youdao_v3_sign(app_key, text, &salt, &curtime, app_secret);
    let from = source_language
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
        .map(map_youdao_lang)
        .unwrap_or_else(|| "auto".to_string());
    let to = map_youdao_lang(target_language);

    let response = post_form_request(
        &url,
        &[
            ("q", text),
            ("from", &from),
            ("to", &to),
            ("appKey", app_key),
            ("salt", &salt),
            ("sign", &sign),
            ("signType", "v3"),
            ("curtime", &curtime),
        ],
        config.timeout_seconds,
    )
    .await?;

    let parsed: YoudaoResponse = serde_json::from_value(response).map_err(|e| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Invalid Youdao Translate response: {e}"),
        )
    })?;
    if parsed.error_code != "0" {
        let kind = match parsed.error_code.as_str() {
            "108" | "202" | "101" => LlmPortErrorKind::Authentication,
            "411" | "412" => LlmPortErrorKind::RateLimited,
            _ => LlmPortErrorKind::Protocol,
        };
        return Err(LlmPortError::new(
            kind,
            format!("Youdao Translate error code {}", parsed.error_code),
        ));
    }

    let translations = parsed.translation.ok_or_else(|| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "Invalid Youdao Translate response: missing translation",
        )
    })?;
    Ok(translations.into_iter().next().unwrap_or_default())
}

pub async fn execute_translation_batch(
    request: &LlmTranslationRequest,
) -> Result<Vec<String>, LlmPortError> {
    let config = &request.config;
    let target = &request.target_language;
    let source = request.source_language.as_deref();

    match config.strategy {
        LlmProviderStrategy::Deepl => {
            translate_deepl(config, request.texts.clone(), target, source).await
        }
        LlmProviderStrategy::MicrosoftTranslator => {
            translate_microsoft(config, request.texts.clone(), target, source).await
        }
        LlmProviderStrategy::LibreTranslate => {
            translate_libre_translate(config, request.texts.clone(), target, source).await
        }
        LlmProviderStrategy::Deeplx => {
            let mut out = Vec::with_capacity(request.texts.len());
            for text in &request.texts {
                out.push(translate_deeplx_single(config, text, target, source).await?);
            }
            Ok(out)
        }
        LlmProviderStrategy::BaiduTranslate => {
            let mut out = Vec::with_capacity(request.texts.len());
            for text in &request.texts {
                out.push(translate_baidu_single(config, text, target, source).await?);
            }
            Ok(out)
        }
        LlmProviderStrategy::YoudaoTranslate => {
            let mut out = Vec::with_capacity(request.texts.len());
            for text in &request.texts {
                out.push(translate_youdao_single(config, text, target, source).await?);
            }
            Ok(out)
        }
        _ => Err(LlmPortError::new(
            LlmPortErrorKind::Unsupported,
            "Unsupported translation strategy",
        )),
    }
}

pub async fn execute_translation_completion(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let batch_request = LlmTranslationRequest {
        config: request.config.clone(),
        texts: vec![request.input.clone()],
        target_language: "en".to_string(),
        source_language: None,
    };
    let translations = execute_translation_batch(&batch_request).await?;
    Ok(StandardLlmResponse {
        text: translations.into_iter().next().unwrap_or_default(),
        thought: None,
        usage: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_rfc1321_and_baidu_official_vector() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5_hex(b"2015063000000001apple143566028812345678"),
            "f89f9594663708c1605f3d736d01d2d4"
        );
    }

    #[test]
    fn youdao_input_truncation_handles_short_and_long_cjk_strings() {
        assert_eq!(youdao_sign_input("hello"), "hello");
        let exact_20 = "12345678901234567890";
        assert_eq!(youdao_sign_input(exact_20), exact_20);

        let long_cjk = "一二三四五六七八九十甲乙丙丁戊己庚辛壬癸子";
        assert_eq!(long_cjk.chars().count(), 21);
        assert_eq!(youdao_sign_input(long_cjk), "一二三四五六七八九十21乙丙丁戊己庚辛壬癸子");
    }

    #[test]
    fn resolves_deepl_free_vs_pro_host_automatically() {
        assert_eq!(
            resolve_deepl_base_url("https://api-free.deepl.com", "abc:fx"),
            "https://api-free.deepl.com"
        );
        assert_eq!(
            resolve_deepl_base_url("https://api-free.deepl.com", "abc-pro-key"),
            "https://api.deepl.com"
        );
        assert_eq!(
            resolve_deepl_base_url("https://custom.proxy.example/deepl", "abc-pro-key"),
            "https://custom.proxy.example/deepl"
        );
    }

    #[test]
    fn maps_chinese_and_common_language_codes() {
        assert_eq!(map_deepl_lang("zh-CN", true), "ZH-HANS");
        assert_eq!(map_deepl_lang("zh-TW", true), "ZH-HANT");
        assert_eq!(map_deepl_lang("zh-CN", false), "ZH");
        assert_eq!(map_microsoft_lang("zh-CN"), "zh-Hans");
        assert_eq!(map_microsoft_lang("zh-TW"), "zh-Hant");
        assert_eq!(map_libre_translate_lang("zh-CN"), "zh");
        assert_eq!(map_libre_translate_lang("zh-TW"), "zt");
        assert_eq!(map_baidu_lang("zh-CN"), "zh");
        assert_eq!(map_baidu_lang("zh-TW"), "cht");
        assert_eq!(map_baidu_lang("ja"), "jp");
        assert_eq!(map_youdao_lang("zh-CN"), "zh-CHS");
        assert_eq!(map_youdao_lang("zh-TW"), "zh-CHT");
    }
}
