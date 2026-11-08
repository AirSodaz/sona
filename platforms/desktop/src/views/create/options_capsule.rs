//! Persistent bottom options capsule and parameter settings popover.
//! Strict 1:1 parity with React `TranscriptionOptions.tsx` and `ParameterSettingsPopover.tsx` (Gap G13).

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{
    COLOR_BG_ELEVATED_LIGHT, COLOR_BG_PRIMARY_LIGHT, COLOR_BG_SECONDARY_LIGHT,
    COLOR_BORDER_HOVER_LIGHT, COLOR_BORDER_LIGHT, COLOR_PROJECT_AMBER, COLOR_TEXT_MUTED_LIGHT,
    COLOR_TEXT_PRIMARY_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_LG, RADIUS_SM,
};
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[inline]
fn icon(name: LucideIcon) -> Icon {
    Icon::default().path(name.path())
}

/// Active audio ingestion surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IngestionSurface {
    #[default]
    Live,
    Batch,
}

/// Transcription display and alignment mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TranscriptionMode {
    #[default]
    Timeline,
    Subtitle,
    Dictation,
}

impl TranscriptionMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Timeline => "Timeline",
            Self::Subtitle => "Subtitle",
            Self::Dictation => "Dictation",
        }
    }
}

/// Shared transcription options state across Live Record, Batch Import, and Options Capsule.
pub struct TranscriptionOptionsModel {
    pub surface: IngestionSurface,
    pub mode: TranscriptionMode,

    // Live surface parameters
    pub live_model_id: String,
    pub live_language: String,
    pub live_diarization: bool,

    // Batch surface parameters
    pub batch_model_id: String,
    pub batch_language: String,
    pub batch_diarization: bool,

    // Shared parameters
    pub enable_timeline: bool,
    pub vad_sensitivity: f32, // 0.2 to 0.9 (default 0.5)

    // UI & Popover state
    pub is_popover_open: bool,
    pub is_loading_catalog: bool,
    pub is_model_selector_open: bool,
    pub is_lang_selector_open: bool,
    pub catalog: Option<sona_core::models::preset_models::ModelCatalogSnapshot>,
}

impl Default for TranscriptionOptionsModel {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptionOptionsModel {
    pub fn new() -> Self {
        Self {
            surface: IngestionSurface::Live,
            mode: TranscriptionMode::Timeline,
            live_model_id: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
            live_language: "auto".to_string(),
            live_diarization: true,
            batch_model_id: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
            batch_language: "auto".to_string(),
            batch_diarization: true,
            enable_timeline: true,
            vad_sensitivity: 0.5,
            is_popover_open: false,
            is_loading_catalog: false,
            is_model_selector_open: false,
            is_lang_selector_open: false,
            catalog: None,
        }
    }
    /// Loads persisted settings from DesktopServices configuration.
    pub fn load_from_services(&mut self, services: &crate::services::DesktopServices) {
        if let Ok(Some(config)) = services.config.load() {
            if let Some(timeline) = config
                .get("enableTimeline")
                .and_then(serde_json::Value::as_bool)
            {
                self.enable_timeline = timeline;
            }
            if let Some(live_diar) = config
                .get("liveSpeakerDiarizationEnabled")
                .and_then(serde_json::Value::as_bool)
            {
                self.live_diarization = live_diar;
            }
            if let Some(batch_diar) = config
                .get("batchSpeakerDiarizationEnabled")
                .and_then(serde_json::Value::as_bool)
            {
                self.batch_diarization = batch_diar;
            }
            if let Some(lang) = config.get("language").and_then(serde_json::Value::as_str) {
                self.live_language = lang.to_string();
                self.batch_language = lang.to_string();
            }
            if let Some(stream_model) = config
                .get("streamingModelPath")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                self.live_model_id = stream_model.to_string();
            }
            if let Some(batch_model) = config
                .get("batchModelPath")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                self.batch_model_id = batch_model.to_string();
            }
            self.coerce_languages();
        }
    }

    /// Fetches model catalog snapshot asynchronously from the backend runtime.
    pub fn refresh_catalog(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_catalog {
            return;
        }
        self.is_loading_catalog = true;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
            let snapshot_res =
                crate::platform::models::preset::get_model_catalog_snapshot(&provider).await;

            this.update(cx, |model, cx| {
                model.is_loading_catalog = false;
                if let Ok(snapshot) = snapshot_res {
                    model.catalog = Some(snapshot);
                    model.coerce_languages();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Sets the active surface (Live vs Batch) and synchronizes language/model coercion.
    pub fn set_surface(&mut self, surface: IngestionSurface, cx: &mut Context<Self>) {
        if self.surface != surface {
            self.surface = surface;
            self.coerce_languages();
            cx.notify();
        }
    }

    /// Ensures configured languages match model capabilities (Auto/Fixed/Selectable).
    pub fn coerce_languages(&mut self) {
        let (model_id, lang_slot) = match self.surface {
            IngestionSurface::Live => (&self.live_model_id, &mut self.live_language),
            IngestionSurface::Batch => (&self.batch_model_id, &mut self.batch_language),
        };

        if let Some(cat) = &self.catalog
            && let Some(m) = cat.models.iter().find(|m| &m.id == model_id)
        {
            match m.language_mode {
                sona_core::models::preset_models::LanguageMode::Auto => {
                    *lang_slot = "auto".to_string();
                }
                sona_core::models::preset_models::LanguageMode::Fixed => {
                    if let Some(first_lang) = m.languages.first() {
                        *lang_slot = first_lang.clone();
                    }
                }
                sona_core::models::preset_models::LanguageMode::Selectable
                | sona_core::models::preset_models::LanguageMode::None => {}
            }
        }
    }

    /// Current active model display label.
    pub fn active_model_label(&self) -> String {
        let id = match self.surface {
            IngestionSurface::Live => &self.live_model_id,
            IngestionSurface::Batch => &self.batch_model_id,
        };
        if let Some(cat) = &self.catalog
            && let Some(m) = cat.models.iter().find(|m| &m.id == id)
        {
            return m.name.clone();
        }
        if id.contains("sense-voice") {
            "SenseVoice Small".to_string()
        } else if id.contains("whisper-turbo") {
            "Whisper Turbo".to_string()
        } else {
            id.split("::").nth(1).unwrap_or(id).to_string()
        }
    }

    /// Current active language display label.
    pub fn active_language_label(&self) -> String {
        let lang = match self.surface {
            IngestionSurface::Live => &self.live_language,
            IngestionSurface::Batch => &self.batch_language,
        };
        match lang.as_str() {
            "auto" => "Auto Detect".to_string(),
            "en" => "English".to_string(),
            "zh" => "Chinese".to_string(),
            "ja" => "Japanese".to_string(),
            "ko" => "Korean".to_string(),
            "yue" => "Cantonese".to_string(),
            "es" => "Spanish".to_string(),
            "fr" => "French".to_string(),
            "de" => "German".to_string(),
            other => other.to_uppercase(),
        }
    }

    /// Check whether diarization is supported by the active model.
    pub fn is_diarization_supported(&self) -> bool {
        let id = match self.surface {
            IngestionSurface::Live => &self.live_model_id,
            IngestionSurface::Batch => &self.batch_model_id,
        };
        // Cloud models without diarization capability return false
        !id.starts_with("cloud::")
    }

    /// Checks if active model enforces a locked language mode.
    pub fn is_language_locked(&self) -> Option<&'static str> {
        let id = match self.surface {
            IngestionSurface::Live => &self.live_model_id,
            IngestionSurface::Batch => &self.batch_model_id,
        };
        if let Some(cat) = &self.catalog
            && let Some(m) = cat.models.iter().find(|m| &m.id == id)
        {
            match m.language_mode {
                sona_core::models::preset_models::LanguageMode::Auto => {
                    return Some("Auto-detect only for this model");
                }
                sona_core::models::preset_models::LanguageMode::Fixed => {
                    return Some("Fixed language model");
                }
                _ => {}
            }
        }
        None
    }

    /// Updates the selected model for a specific ingestion surface.
    pub fn set_model(&mut self, surface: IngestionSurface, model_id: String) {
        match surface {
            IngestionSurface::Live => self.live_model_id = model_id,
            IngestionSurface::Batch => self.batch_model_id = model_id,
        }
        self.coerce_languages();
    }

    /// Updates the selected language for a specific ingestion surface.
    pub fn set_language(&mut self, surface: IngestionSurface, lang: String) {
        match surface {
            IngestionSurface::Live => self.live_language = lang,
            IngestionSurface::Batch => self.batch_language = lang,
        }
    }

    /// Updates the diarization preference for a specific ingestion surface.
    pub fn set_diarization(&mut self, surface: IngestionSurface, diarize: bool) {
        match surface {
            IngestionSurface::Live => self.live_diarization = diarize,
            IngestionSurface::Batch => self.batch_diarization = diarize,
        }
    }

    /// Sets the VAD sensitivity clamped between 0.0 and 1.0.
    pub fn set_vad_sensitivity(&mut self, sensitivity: f32) {
        self.vad_sensitivity = sensitivity.clamp(0.0, 1.0);
    }
}

/// 4-pill persistent Airy capsule rendering in `#panel-footer`.
pub struct OptionsCapsule {
    pub model: Entity<TranscriptionOptionsModel>,
}

impl OptionsCapsule {
    pub fn new(model: Entity<TranscriptionOptionsModel>) -> Self {
        Self { model }
    }

    pub fn render<V: 'static>(&mut self, disabled: bool, cx: &mut Context<V>) -> impl IntoElement {
        let model = self.model.read(cx);
        let is_open = model.is_popover_open && !disabled;
        let model_label = model.active_model_label();
        let lang_label = model.active_language_label();
        let _mode_label = model.mode.label();
        let diarization_supported = model.is_diarization_supported();
        let is_lang_locked = model.is_language_locked();
        let enable_timeline = model.enable_timeline;
        let is_model_selector_open = model.is_model_selector_open;
        let is_lang_selector_open = model.is_lang_selector_open;
        let _vad_sens = model.vad_sensitivity;
        let current_surface = model.surface;
        let active_diarization = match current_surface {
            IngestionSurface::Live => model.live_diarization,
            IngestionSurface::Batch => model.batch_diarization,
        };

        // Available models list
        let available_models: Vec<(String, String)> = if let Some(cat) = &model.catalog {
            cat.models
                .iter()
                .map(|m| (m.id.clone(), m.name.clone()))
                .collect()
        } else {
            vec![
                (
                    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string(),
                    "SenseVoice Small".to_string(),
                ),
                (
                    "sherpa-onnx-whisper-turbo".to_string(),
                    "Whisper Turbo".to_string(),
                ),
            ]
        };

        div()
            .id("options-container")
            .relative()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            // ==========================================
            // Dropup Parameter Settings Popover (310px width)
            // ==========================================
            .when(is_open, |this| {
                let model_handle = self.model.clone();
                this.child(
                    div()
                        .id("parameter-settings-popover")
                        .absolute()
                        .bottom(px(44.0))
                        .w(px(310.0))
                        .p(px(16.0))
                        .rounded(RADIUS_LG)
                        .bg(COLOR_BG_ELEVATED_LIGHT)
                        .border_1()
                        .border_color(COLOR_BORDER_LIGHT)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap(px(14.0))
                        // Field 1: Model Selection
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(cx.theme().foreground)
                                                .child("Model"),
                                        )
                                        .child(
                                            div()
                                                .id("btn-popover-model")
                                                .flex()
                                                .items_center()
                                                .gap(px(6.0))
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .rounded(RADIUS_SM)
                                                .bg(cx.theme().secondary.opacity(0.6))
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .cursor_pointer()
                                                .child(icon(LucideIcon::Cpu).size(px(12.0)))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .max_w(px(130.0))
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .child(model_label.clone()),
                                                )
                                                .child(icon(LucideIcon::ChevronDown).size(px(12.0)))
                                                .on_click(cx.listener({
                                                    let model_handle = model_handle.clone();
                                                    move |_view, _, _, cx| {
                                                        model_handle.update(cx, |m, cx| {
                                                            m.is_model_selector_open =
                                                                !m.is_model_selector_open;
                                                            cx.notify();
                                                        });
                                                    }
                                                })),
                                        ),
                                )
                                .when(is_model_selector_open, |this| {
                                    let model_handle = model_handle.clone();
                                    this.child(
                                        div()
                                            .id("model-selection-list")
                                            .p(px(4.0))
                                            .rounded(RADIUS_SM)
                                            .bg(cx.theme().background)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .children(available_models.into_iter().map(
                                                |(mid, mname)| {
                                                    let is_selected = mid == model.live_model_id;
                                                    let mid_clone = mid.clone();
                                                    let model_handle = model_handle.clone();

                                                    div()
                                                        .id(ElementId::from(format!(
                                                            "model-opt-{}",
                                                            mid
                                                        )))
                                                        .px(px(8.0))
                                                        .py(px(4.0))
                                                        .rounded(RADIUS_SM)
                                                        .cursor_pointer()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .bg(if is_selected {
                                                            cx.theme().secondary
                                                        } else {
                                                            gpui_kit::transparent_black()
                                                        })
                                                        .child(
                                                            div().text_size(px(11.5)).child(mname),
                                                        )
                                                        .when(is_selected, |this| {
                                                            this.child(
                                                                icon(LucideIcon::Check)
                                                                    .size(px(12.0))
                                                                    .text_color(cx.theme().primary),
                                                            )
                                                        })
                                                        .on_click(cx.listener(
                                                            move |_view, _, _, cx| {
                                                                let mid_target = mid_clone.clone();
                                                                let services = cx.global::<GlobalDesktopServices>().0.clone();
                                                                model_handle.update(cx, |m, cx| {
                                                                    let key = match m.surface {
                                                                        IngestionSurface::Live => {
                                                                            m.live_model_id = mid_target.clone();
                                                                            "streamingModelPath"
                                                                        }
                                                                        IngestionSurface::Batch => {
                                                                            m.batch_model_id = mid_target.clone();
                                                                            "batchModelPath"
                                                                        }
                                                                    };
                                                                    let _ = crate::platform::agent_control::facade::settings::update_setting(
                                                                        &services,
                                                                        key.to_string(),
                                                                        serde_json::Value::String(mid_target),
                                                                    );
                                                                    m.coerce_languages();
                                                                    m.is_model_selector_open =
                                                                        false;
                                                                    cx.notify();
                                                                });
                                                            },
                                                        ))
                                                },
                                            )),
                                    )
                                }),
                        )
                        // Field 2: Timeline Mode
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(cx.theme().foreground)
                                                .child("Timeline Mode"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Align timestamps with audio playback"),
                                        ),
                                )
                                .child(
                                    Switch::new("switch-timeline")
                                        .checked(enable_timeline)
                                        .on_change(cx.listener({
                                            let model_handle = model_handle.clone();
                                            move |_view, checked, _, cx| {
                                                let val = *checked;
                                                let services = cx.global::<GlobalDesktopServices>().0.clone();
                                                let _ = crate::platform::agent_control::facade::settings::update_setting(
                                                    &services,
                                                    "enableTimeline".to_string(),
                                                    serde_json::Value::Bool(val),
                                                );
                                                model_handle.update(cx, |m, cx| {
                                                    m.enable_timeline = val;
                                                    cx.notify();
                                                });
                                            }
                                        })),
                                ),
                        )
                        // Field 3: Speaker Diarization
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(cx.theme().foreground)
                                                .child("Speaker Diarization"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(if !diarization_supported {
                                                    COLOR_PROJECT_AMBER
                                                } else {
                                                    cx.theme().muted_foreground
                                                })
                                                .child(if !diarization_supported {
                                                    "Model does not support speaker diarization"
                                                } else {
                                                    "Distinguish speakers and identify voices"
                                                }),
                                        ),
                                )
                                .child(
                                    Switch::new("switch-diarization")
                                        .checked(active_diarization && diarization_supported)
                                        .disabled(!diarization_supported)
                                        .on_change(cx.listener({
                                            let model_handle = model_handle.clone();
                                            move |_view, checked, _, cx| {
                                                let val = *checked;
                                                let services = cx.global::<GlobalDesktopServices>().0.clone();
                                                model_handle.update(cx, |m, cx| {
                                                    let key = match m.surface {
                                                        IngestionSurface::Live => {
                                                            m.live_diarization = val;
                                                            "liveSpeakerDiarizationEnabled"
                                                        }
                                                        IngestionSurface::Batch => {
                                                            m.batch_diarization = val;
                                                            "batchSpeakerDiarizationEnabled"
                                                        }
                                                    };
                                                    let _ = crate::platform::agent_control::facade::settings::update_setting(
                                                        &services,
                                                        key.to_string(),
                                                        serde_json::Value::Bool(val),
                                                    );
                                                    cx.notify();
                                                });
                                            }
                                        })),
                                ),
                        )
                        // Field 4: Language Selection
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(cx.theme().foreground)
                                                        .child("Language"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(is_lang_locked.unwrap_or(
                                                            "Language override or auto-detect",
                                                        )),
                                                ),
                                        )
                                        .child(if let Some(_locked_hint) = is_lang_locked {
                                            div()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .rounded(RADIUS_SM)
                                                .bg(cx.theme().secondary.opacity(0.4))
                                                .text_size(px(11.0))
                                                .text_color(cx.theme().muted_foreground)
                                                .child(lang_label.clone())
                                                .into_any_element()
                                        } else {
                                            div()
                                                .id("btn-popover-lang")
                                                .flex()
                                                .items_center()
                                                .gap(px(6.0))
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .rounded(RADIUS_SM)
                                                .bg(cx.theme().secondary.opacity(0.6))
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .cursor_pointer()
                                                .child(icon(LucideIcon::Globe).size(px(12.0)))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .child(lang_label.clone()),
                                                )
                                                .child(icon(LucideIcon::ChevronDown).size(px(12.0)))
                                                .on_click(cx.listener({
                                                    let model_handle = model_handle.clone();
                                                    move |_view, _, _, cx| {
                                                        model_handle.update(cx, |m, cx| {
                                                            m.is_lang_selector_open =
                                                                !m.is_lang_selector_open;
                                                            cx.notify();
                                                        });
                                                    }
                                                }))
                                                .into_any_element()
                                        }),
                                )
                                .when(is_lang_selector_open && is_lang_locked.is_none(), |this| {
                                    let model_handle = model_handle.clone();
                                    let langs = [
                                        ("auto", "Auto Detect"),
                                        ("en", "English"),
                                        ("zh", "Chinese"),
                                        ("ja", "Japanese"),
                                        ("ko", "Korean"),
                                        ("yue", "Cantonese"),
                                        ("es", "Spanish"),
                                        ("fr", "French"),
                                        ("de", "German"),
                                    ];

                                    this.child(
                                        div()
                                            .id("lang-selection-list")
                                            .p(px(4.0))
                                            .rounded(RADIUS_SM)
                                            .bg(cx.theme().background)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .children(langs.into_iter().map(|(code, name)| {
                                                let is_selected = match current_surface {
                                                    IngestionSurface::Live => {
                                                        model.live_language == code
                                                    }
                                                    IngestionSurface::Batch => {
                                                        model.batch_language == code
                                                    }
                                                };
                                                let model_handle = model_handle.clone();
                                                let target_code = code.to_string();

                                                div()
                                                    .id(ElementId::from(format!("lang-opt-{code}")))
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(RADIUS_SM)
                                                    .cursor_pointer()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .bg(if is_selected {
                                                        cx.theme().secondary
                                                    } else {
                                                        gpui_kit::transparent_black()
                                                    })
                                                    .child(div().text_size(px(11.5)).child(name))
                                                    .when(is_selected, |this| {
                                                        this.child(
                                                            icon(LucideIcon::Check)
                                                                .size(px(12.0))
                                                                .text_color(cx.theme().primary),
                                                        )
                                                    })
                                                    .on_click(cx.listener(
                                                        move |_view, _, _, cx| {
                                                            let code = target_code.clone();
                                                            let services = cx.global::<GlobalDesktopServices>().0.clone();
                                                            model_handle.update(cx, |m, cx| {
                                                                match m.surface {
                                                                    IngestionSurface::Live => {
                                                                        m.live_language = code.clone();
                                                                    }
                                                                    IngestionSurface::Batch => {
                                                                        m.batch_language = code.clone();
                                                                    }
                                                                }
                                                                let _ = crate::platform::agent_control::facade::settings::update_setting(
                                                                    &services,
                                                                    "language".to_string(),
                                                                    serde_json::Value::String(code),
                                                                );
                                                                m.is_lang_selector_open = false;
                                                                cx.notify();
                                                            });
                                                        },
                                                    ))
                                            })),
                                    )
                                }),
                        ),
                )
            })
            // ==========================================
            // Persistent Airy Capsule Trigger (matching React TranscriptionOptions.tsx 1:1)
            // ==========================================
            .child(
                div()
                    .id("btn-parameter-settings")
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .px(px(14.0))
                    .py(px(6.0))
                    .h(px(32.0))
                    .rounded(px(20.0))
                    .bg(if is_open {
                        COLOR_BG_SECONDARY_LIGHT
                    } else {
                        COLOR_BG_PRIMARY_LIGHT
                    })
                    .border_1()
                    .border_color(if is_open {
                        COLOR_BORDER_HOVER_LIGHT
                    } else {
                        COLOR_BORDER_LIGHT
                    })
                    .shadow_xs()
                    .tooltip({
                        let tooltip_text = format!("Preset: {}, {}", model_label, lang_label);
                        move |window, cx| Tooltip::new(tooltip_text.clone()).build(window, cx)
                    })
                    .when(disabled, |this| this.opacity(0.55).cursor_default())
                    .when(!disabled, |this| {
                        let model_handle = self.model.clone();
                        this.cursor_pointer()
                            .hover(|s| s.bg(COLOR_BG_SECONDARY_LIGHT))
                            .on_click(cx.listener(move |_view, _, _, cx| {
                                let services = cx.global::<GlobalDesktopServices>().0.clone();
                                model_handle.update(cx, |m, cx| {
                                    m.is_popover_open = !m.is_popover_open;
                                    if m.is_popover_open {
                                        m.load_from_services(&services);
                                        m.refresh_catalog(cx);
                                    }
                                    cx.notify();
                                });
                            }))
                    })
                    // 1. Status Accent Dot (6x6 circle)
                    .child(
                        div()
                            .id("parameter-settings-status-dot")
                            .size(px(6.0))
                            .rounded_full()
                            .bg(COLOR_TEXT_PRIMARY_LIGHT),
                    )
                    // 2. Summary: Model / Language
                    .child(
                        div()
                            .id("parameter-settings-capsule-summary")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .id("parameter-settings-summary-model")
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                    .max_w(px(130.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(model_label),
                            )
                            .child(
                                div()
                                    .id("parameter-settings-summary-divider")
                                    .text_size(px(12.0))
                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                    .child("/"),
                            )
                            .child(
                                div()
                                    .id("parameter-settings-summary-lang")
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                    .child(lang_label),
                            ),
                    )
                    // 3. Chevron Up/Down (14x14)
                    .child(
                        icon(if is_open {
                            LucideIcon::ChevronDown
                        } else {
                            LucideIcon::ChevronUp
                        })
                        .size(px(14.0))
                        .text_color(if is_open {
                            COLOR_TEXT_PRIMARY_LIGHT
                        } else {
                            COLOR_TEXT_MUTED_LIGHT
                        }),
                    ),
            )
    }
}
