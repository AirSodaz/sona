//! LlamaCpp translation adapter implementing TranslationEnginePort.

use crate::llm::LlamaCppLlmEngine;
use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{LlmCompletionOptions, LlmCompletionRequest};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortErrorKind;
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};
use std::sync::Arc;

pub struct LlamaCppTranslationAdapter {
    engine: Arc<LlamaCppLlmEngine>,
    model_name: String,
}

impl LlamaCppTranslationAdapter {
    pub fn new(engine: Arc<LlamaCppLlmEngine>, model_name: impl Into<String>) -> Self {
        Self {
            engine,
            model_name: model_name.into(),
        }
    }
}

#[async_trait]
impl TranslationEnginePort for LlamaCppTranslationAdapter {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        let src = source_lang.trim();
        let tgt = target_lang.trim();
        if src.is_empty() || tgt.is_empty() {
            return Err(TranslationPortError::invalid_language(
                "Source and target languages cannot be empty",
            ));
        }

        let trimmed = text.trim();
        if trimmed.is_empty() || src.eq_ignore_ascii_case(tgt) {
            return Ok(trimmed.to_string());
        }
        let (system_prompt, input) =
            build_specialized_translation_prompt(&self.model_name, trimmed, src, tgt);
        let request = LlmCompletionRequest {
            config: LlmConfig {
                provider: LlmProvider::Builtin(BuiltinLlmProvider::Local),
                strategy: LlmProviderStrategy::Local,
                base_url: String::new(),
                api_key: String::new(),
                model: self.model_name.clone(),
                api_path: None,
                api_version: None,
                temperature: Some(0.0),
                reasoning_enabled: Some(false),
                reasoning_level: None,
                timeout_seconds: Some(30),
            },
            system_prompt,
            input,
            options: LlmCompletionOptions {
                max_output_tokens: Some((trimmed.len() as u64).saturating_mul(2).clamp(256, 2048)),
                ..Default::default()
            },
            source: None,
        };

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let response = match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.engine.execute_completion_internal(request, Some(tx)),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(llm_err)) => {
                return Err(match llm_err.kind {
                    LlmPortErrorKind::Unavailable => {
                        TranslationPortError::model_not_ready(llm_err.message)
                    }
                    _ => TranslationPortError::inference_failed(llm_err.message),
                });
            }
            Err(_) => {
                return Err(TranslationPortError::engine_failed(
                    "Translation timed out after 30 seconds",
                ));
            }
        };
        Ok(strip_translation_tags(&response.text))
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::LlamaCpp
    }
}

fn strip_translation_tags(text: &str) -> String {
    let mut result = text.trim().to_string();
    if let Some(idx) = result.find("<translation>") {
        result = result[idx + "<translation>".len()..].to_string();
    }
    if let Some(idx) = result.rfind("</translation>") {
        result = result[..idx].to_string();
    }
    result.trim().to_string()
}

pub fn resolve_language_name(code: &str, in_chinese: bool) -> Option<&'static str> {
    let normalized = code.trim().to_lowercase();
    let base = normalized.split(['-', '_']).next().unwrap_or(&normalized);
    let name = match (base, in_chinese) {
        ("zh" | "chinese" | "cmn" | "zho" | "chi", true) => "中文",
        ("zh" | "chinese" | "cmn" | "zho" | "chi", false) => "Chinese",
        ("en" | "english" | "eng", true) => "英语",
        ("en" | "english" | "eng", false) => "English",
        ("ja" | "japanese" | "jpn", true) => "日语",
        ("ja" | "japanese" | "jpn", false) => "Japanese",
        ("ko" | "korean" | "kor", true) => "韩语",
        ("ko" | "korean" | "kor", false) => "Korean",
        ("fr" | "french" | "fra" | "fre", true) => "法语",
        ("fr" | "french" | "fra" | "fre", false) => "French",
        ("de" | "german" | "deu" | "ger", true) => "德语",
        ("de" | "german" | "deu" | "ger", false) => "German",
        ("es" | "spanish" | "spa", true) => "西班牙语",
        ("es" | "spanish" | "spa", false) => "Spanish",
        ("ru" | "russian" | "rus", true) => "俄语",
        ("ru" | "russian" | "rus", false) => "Russian",
        ("it" | "italian" | "ita", true) => "意大利语",
        ("it" | "italian" | "ita", false) => "Italian",
        ("pt" | "portuguese" | "por", true) => "葡萄牙语",
        ("pt" | "portuguese" | "por", false) => "Portuguese",
        ("vi" | "vietnamese" | "vie", true) => "越南语",
        ("vi" | "vietnamese" | "vie", false) => "Vietnamese",
        ("th" | "thai" | "tha", true) => "泰语",
        ("th" | "thai", false) => "Thai",
        ("ar" | "arabic" | "ara", true) => "阿拉伯语",
        ("ar" | "arabic" | "ara", false) => "Arabic",
        ("yue" | "cantonese", true) => "粤语",
        ("yue" | "cantonese", false) => "Cantonese",
        ("nl" | "dutch" | "nld" | "dut", true) => "荷兰语",
        ("nl" | "dutch" | "nld" | "dut", false) => "Dutch",
        ("pl" | "polish" | "pol", true) => "波兰语",
        ("pl" | "polish" | "pol", false) => "Polish",
        ("tr" | "turkish" | "tur", true) => "土耳其语",
        ("tr" | "turkish" | "tur", false) => "Turkish",
        ("id" | "indonesian" | "ind", true) => "印尼语",
        ("id" | "indonesian" | "ind", false) => "Indonesian",
        ("hi" | "hindi" | "hin", true) => "印地语",
        ("hi" | "hindi" | "hin", false) => "Hindi",
        _ => return None,
    };
    Some(name)
}

fn is_chinese_lang(lang: &str) -> bool {
    let s = lang.trim().to_lowercase();
    s == "zh"
        || s.starts_with("zh-")
        || s.starts_with("zh_")
        || s == "chinese"
        || s == "中文"
        || s == "cmn"
}

pub fn build_specialized_translation_prompt(
    model_name: &str,
    text: &str,
    src_lang: &str,
    tgt_lang: &str,
) -> (Option<String>, String) {
    let lower_name = model_name.to_lowercase();

    if lower_name.contains("index-translate") {
        if is_chinese_lang(tgt_lang) {
            (None, format!("将下面的文本翻译成中文：\n{text}"))
        } else if tgt_lang.trim().eq_ignore_ascii_case("en")
            || tgt_lang.trim().eq_ignore_ascii_case("english")
        {
            (
                None,
                format!("Translate the following text into English:\n{text}"),
            )
        } else if let Some(target_name) = resolve_language_name(tgt_lang, false) {
            (
                None,
                format!("Translate the following text into {target_name}:\n{text}"),
            )
        } else {
            let system_prompt = format!(
                "You are a professional real-time translator. Translate the text inside <text_to_translate> from {src_lang} to {tgt_lang}. Output only the translation without any explanation, markdown formatting, or commentary."
            );
            (
                Some(system_prompt),
                format!("<text_to_translate>\n{text}\n</text_to_translate>"),
            )
        }
    } else if lower_name.contains("hy-mt") || lower_name.contains("hunyuan-mt") {
        if is_chinese_lang(src_lang) || is_chinese_lang(tgt_lang) {
            if is_chinese_lang(tgt_lang) {
                (None, format!("把下面的文本翻译成中文：\n{text}"))
            } else if let Some(target_name) = resolve_language_name(tgt_lang, true) {
                (None, format!("把下面的文本翻译成{target_name}：\n{text}"))
            } else {
                let system_prompt = format!(
                    "You are a professional real-time translator. Translate the text inside <text_to_translate> from {src_lang} to {tgt_lang}. Output only the translation without any explanation, markdown formatting, or commentary."
                );
                (
                    Some(system_prompt),
                    format!("<text_to_translate>\n{text}\n</text_to_translate>"),
                )
            }
        } else if let Some(target_name) = resolve_language_name(tgt_lang, false) {
            (
                None,
                format!("Translate the following text into {target_name}:\n{text}"),
            )
        } else {
            let system_prompt = format!(
                "You are a professional real-time translator. Translate the text inside <text_to_translate> from {src_lang} to {tgt_lang}. Output only the translation without any explanation, markdown formatting, or commentary."
            );
            (
                Some(system_prompt),
                format!("<text_to_translate>\n{text}\n</text_to_translate>"),
            )
        }
    } else {
        let system_prompt = format!(
            "You are a professional real-time translator. Translate the text inside <text_to_translate> from {src_lang} to {tgt_lang}. Output only the translation without any explanation, markdown formatting, or commentary."
        );
        (
            Some(system_prompt),
            format!("<text_to_translate>\n{text}\n</text_to_translate>"),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::translation::TranslationPortErrorKind;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_adapter_properties_and_passthrough() {
        let engine = Arc::new(LlamaCppLlmEngine::new());
        let adapter = LlamaCppTranslationAdapter::new(engine, "test-model");

        // engine_kind
        assert_eq!(adapter.engine_kind(), TranslationEngineKind::LlamaCpp);

        // empty language error
        let err1 = adapter.translate("hello", "", "zh").await.unwrap_err();
        assert_eq!(err1.kind, TranslationPortErrorKind::InvalidLanguage);

        let err2 = adapter.translate("hello", "en", "   ").await.unwrap_err();
        assert_eq!(err2.kind, TranslationPortErrorKind::InvalidLanguage);

        // whitespace passthrough
        let res = adapter.translate("   ", "en", "zh").await.unwrap();
        assert_eq!(res, "");

        // identical language passthrough
        let res2 = adapter.translate("Hello world", "en", "EN").await.unwrap();
        assert_eq!(res2, "Hello world");

        let res3 = adapter
            .translate("Hello world", "  en  ", "en")
            .await
            .unwrap();
        assert_eq!(res3, "Hello world");
    }

    #[test]
    fn test_strip_translation_tags() {
        assert_eq!(
            strip_translation_tags("<translation>Hello world</translation>"),
            "Hello world"
        );
        assert_eq!(
            strip_translation_tags("  <translation>\n你好世界\n</translation>  "),
            "你好世界"
        );
        assert_eq!(
            strip_translation_tags("Plain text without tags"),
            "Plain text without tags"
        );
    }

    #[test]
    fn test_build_specialized_translation_prompt_index_translate() {
        // Target zh
        let (sys, input) = build_specialized_translation_prompt(
            "Index-Translate-2B.Q4_K_M.gguf",
            "Hello world",
            "en",
            "zh",
        );
        assert!(sys.is_none());
        assert_eq!(input, "将下面的文本翻译成中文：\nHello world");

        // Target en
        let (sys, input) = build_specialized_translation_prompt(
            "IndexTeam/Index-Translate-9B",
            "你好世界",
            "zh",
            "en",
        );
        assert!(sys.is_none());
        assert_eq!(
            input,
            "Translate the following text into English:\n你好世界"
        );

        // Target ja
        let (sys, input) =
            build_specialized_translation_prompt("index-translate-2b", "你好世界", "zh", "ja");
        assert!(sys.is_none());
        assert_eq!(
            input,
            "Translate the following text into Japanese:\n你好世界"
        );
    }

    #[test]
    fn test_build_specialized_translation_prompt_hunyuan_mt() {
        // Target zh
        let (sys, input) = build_specialized_translation_prompt(
            "HY-MT1.5-1.8B-Q4_K_M.gguf",
            "Hello world",
            "en",
            "zh",
        );
        assert!(sys.is_none());
        assert_eq!(input, "把下面的文本翻译成中文：\nHello world");

        // Source zh, target ja
        let (sys, input) =
            build_specialized_translation_prompt("tencent/Hy-MT2-7B", "你好世界", "zh", "ja");
        assert!(sys.is_none());
        assert_eq!(input, "把下面的文本翻译成日语：\n你好世界");

        // Non-Chinese pair (en -> fr)
        let (sys, input) =
            build_specialized_translation_prompt("hy-mt2-7b", "Hello world", "en", "fr");
        assert!(sys.is_none());
        assert_eq!(
            input,
            "Translate the following text into French:\nHello world"
        );
    }

    #[test]
    fn test_build_specialized_translation_prompt_generic_fallback() {
        let (sys, input) = build_specialized_translation_prompt(
            "Qwen3.5-4B-Q4_K_M.gguf",
            "Hello world",
            "en",
            "zh",
        );
        assert_eq!(
            sys,
            Some(
                "You are a professional real-time translator. Translate the text inside <text_to_translate> from en to zh. Output only the translation without any explanation, markdown formatting, or commentary.".to_string()
            )
        );
        assert_eq!(
            input,
            "<text_to_translate>\nHello world\n</text_to_translate>"
        );
    }

    #[test]
    fn test_resolve_language_name() {
        assert_eq!(resolve_language_name("zh", true), Some("中文"));
        assert_eq!(resolve_language_name("zh-CN", true), Some("中文"));
        assert_eq!(resolve_language_name("zh", false), Some("Chinese"));
        assert_eq!(resolve_language_name("en", true), Some("英语"));
        assert_eq!(resolve_language_name("en", false), Some("English"));
        assert_eq!(resolve_language_name("ja", true), Some("日语"));
        assert_eq!(resolve_language_name("ja", false), Some("Japanese"));
        assert_eq!(resolve_language_name("ko", true), Some("韩语"));
        assert_eq!(resolve_language_name("ko", false), Some("Korean"));
        assert_eq!(resolve_language_name("fr", false), Some("French"));
        assert_eq!(resolve_language_name("de", false), Some("German"));
        assert_eq!(resolve_language_name("es", false), Some("Spanish"));
        assert_eq!(resolve_language_name("ru", false), Some("Russian"));
        assert_eq!(resolve_language_name("nl", true), Some("荷兰语"));
        assert_eq!(resolve_language_name("Dutch", false), Some("Dutch"));
        assert_eq!(resolve_language_name("unsupported_xyz", true), None);
    }

    #[test]
    fn test_build_specialized_translation_prompt_unmapped_target_fallback() {
        // Unmapped target language falls back safely to generic prompt rather than corrupting target
        let (sys, input) = build_specialized_translation_prompt(
            "Index-Translate-2B.Q4_K_M.gguf",
            "Hello world",
            "en",
            "xyz_unmapped",
        );
        assert!(sys.is_some());
        assert_eq!(
            input,
            "<text_to_translate>\nHello world\n</text_to_translate>"
        );
    }
}
