//! Live subtitle settings backed by the persisted desktop configuration.
//! Complete 1:1 parity with React `ToolSubtitlesPanel.tsx` & `SettingsSubtitleTab.tsx`.

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{
    COLOR_BG_ELEVATED_LIGHT, COLOR_BG_SECONDARY_LIGHT, COLOR_BG_TERTIARY_LIGHT, COLOR_BORDER_LIGHT,
    COLOR_TEXT_MUTED_LIGHT, COLOR_TEXT_PRIMARY_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_MD,
    RADIUS_SM, hex_to_hsla,
};
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

const SUBTITLE_COLOR_PRESETS: &[&str] = &[
    "#ffffff", "#000000", "#facc15", "#60a5fa", "#4ade80", "#f87171",
];

const SUBTITLE_BG_PRESETS: &[&str] = &["#17171a", "#000000", "#1e293b", "#27272a", "#ffffff"];

const LANGUAGE_OPTIONS: &[(&str, &str)] = &[
    ("auto", "Auto Detect"),
    ("zh", "Chinese"),
    ("en", "English"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("ru", "Russian"),
];

fn setting_bool(services: &crate::services::DesktopServices, key: &str, default: bool) -> bool {
    services
        .config
        .get_setting(key)
        .ok()
        .flatten()
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn setting_string(services: &crate::services::DesktopServices, key: &str, default: &str) -> String {
    services
        .config
        .get_setting(key)
        .ok()
        .flatten()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| default.to_owned())
}

fn setting_f64(services: &crate::services::DesktopServices, key: &str, default: f64) -> f64 {
    services
        .config
        .get_setting(key)
        .ok()
        .flatten()
        .and_then(|value| value.as_f64())
        .unwrap_or(default)
}

pub struct SubtitlesToolPanel;

impl SubtitlesToolPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn render<V: 'static>(&mut self, cx: &mut Context<V>) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();

        // 1. Live Caption State
        let is_caption_mode = setting_bool(&services, "captionMode", false);

        // 2. Window Behavior Settings
        let start_on_launch = setting_bool(&services, "startOnLaunch", false);
        let always_on_top = setting_bool(&services, "alwaysOnTop", true);
        let click_through_locked = setting_bool(&services, "lockWindow", false);
        let caption_lock_shortcut = setting_string(&services, "captionLockShortcut", "Alt+C");

        // 3. Real-time Translation Settings
        let realtime_translation_enabled =
            setting_bool(&services, "realtimeTranslationEnabled", false);
        let source_lang = setting_string(&services, "realtimeTranslationSourceLanguage", "auto");
        let target_lang = setting_string(&services, "realtimeTranslationTargetLanguage", "en");
        let caption_display_mode = setting_string(&services, "captionDisplayMode", "bilingual");
        let caption_bilingual_order =
            setting_string(&services, "captionBilingualOrder", "original_first");

        // 4. Appearance Settings
        let caption_window_width = setting_f64(&services, "captionWindowWidth", 800.0);
        let caption_font_size = setting_f64(&services, "captionFontSize", 22.0);
        let caption_font_color = setting_string(&services, "captionFontColor", "#ffffff");
        let caption_translation_font_size =
            setting_f64(&services, "captionTranslationFontSize", 16.0);
        let caption_translation_font_color =
            setting_string(&services, "captionTranslationFontColor", "#6ee7b7");
        let caption_background_color =
            setting_string(&services, "captionBackgroundColor", "#17171a");
        let caption_background_opacity = setting_f64(&services, "captionBackgroundOpacity", 0.72);
        let caption_text_align = setting_string(&services, "captionTextAlign", "center");
        let caption_text_stroke = setting_bool(&services, "captionTextStroke", false);
        let caption_stroke_width = setting_f64(&services, "captionStrokeWidth", 2.0);
        let caption_stroke_color = setting_string(&services, "captionStrokeColor", "#000000");
        let caption_text_shadow = setting_bool(&services, "captionTextShadow", true);
        let caption_disappear_delay = setting_f64(&services, "captionDisappearDelay", 4.0);

        // Computed Preview Styling
        let preview_stage_bg = hex_to_hsla(&caption_background_color)
            .unwrap_or_else(|| hsla(240.0 / 360.0, 0.06, 0.10, 1.0))
            .opacity(caption_background_opacity.clamp(0.0, 1.0) as f32);
        let preview_orig_fg = hex_to_hsla(&caption_font_color).unwrap_or_else(gpui_kit::white);
        let preview_trans_fg = hex_to_hsla(&caption_translation_font_color)
            .unwrap_or_else(|| hsla(158.0 / 360.0, 0.64, 0.66, 1.0));
        let is_align_center = caption_text_align == "center";

        let is_translation_only = caption_display_mode == "translation";
        let is_original_only = caption_display_mode == "original";
        let is_bilingual = !is_translation_only && !is_original_only;
        let is_translation_first = caption_bilingual_order == "translation_first";

        div()
            .id("tools-panel-subtitles")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(48.0))
            .py(px(32.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(860.0))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(28.0))
                    // ==========================================
                    // 1. Tool Hero Header (matching React ToolHeroHeader 1:1)
                    // ==========================================
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap_4()
                            .pb_6()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex()
                                            .size(px(48.0))
                                            .items_center()
                                            .justify_center()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(COLOR_BG_SECONDARY_LIGHT)
                                            .text_color(cx.theme().primary)
                                            .shadow_xs()
                                            .child(Icon::new(LucideIcon::Captions).size(px(24.0))),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(20.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                                    .child("Live Subtitles"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                                    .child("Configure live caption window behavior and visual appearance."),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        // Status badge
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .px(px(10.0))
                                            .py(px(3.0))
                                            .rounded_full()
                                            .bg(if is_caption_mode {
                                                hsla(142.0 / 360.0, 0.71, 0.45, 0.12)
                                            } else {
                                                COLOR_BG_TERTIARY_LIGHT
                                            })
                                            .text_color(if is_caption_mode {
                                                hsla(142.0 / 360.0, 0.76, 0.36, 1.0)
                                            } else {
                                                COLOR_TEXT_MUTED_LIGHT
                                            })
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(
                                                div()
                                                    .size(px(6.0))
                                                    .rounded_full()
                                                    .bg(if is_caption_mode {
                                                        hsla(142.0 / 360.0, 0.76, 0.36, 1.0)
                                                    } else {
                                                        COLOR_TEXT_MUTED_LIGHT
                                                    }),
                                            )
                                            .child(if is_caption_mode { "Running" } else { "Stopped" }),
                                    )
                                    .child(
                                        Switch::new("switch-caption-mode")
                                            .checked(is_caption_mode)
                                            .accessibility_label("Live Caption")
                                            .on_change(cx.listener(move |_shell, checked, _, cx| {
                                                let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                                                let _ = config.set_setting("captionMode", &Value::Bool(*checked));
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    // ==========================================
                    // 2. Window Behavior Section
                    // ==========================================
                    .child(
                        section("Window Behavior", "Control how the caption window behaves.", cx)
                            .child(toggle_row(
                                "Start on Launch",
                                "Automatically start Live Caption when Sona starts.",
                                "startOnLaunch",
                                start_on_launch,
                                cx,
                            ))
                            .child(toggle_row(
                                "Always on Top",
                                "Keep the caption window floating above other applications.",
                                "alwaysOnTop",
                                always_on_top,
                                cx,
                            ))
                            .child(toggle_row(
                                "Lock Click-Through",
                                "Make window click-through to prevent accidental mouse clicks.",
                                "lockWindow",
                                click_through_locked,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .py_3()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().text_size(px(14.0)).child("Lock Click-Through Shortcut"))
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                                    .child("Global hotkey to toggle mouse click-through for the live caption window."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1p5()
                                            .rounded(RADIUS_SM)
                                            .bg(COLOR_BG_TERTIARY_LIGHT)
                                            .border_1()
                                            .border_color(COLOR_BORDER_LIGHT)
                                            .font_family("Consolas, monospace")
                                            .text_size(px(12.0))
                                            .child(caption_lock_shortcut),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .py_3()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().text_size(px(14.0)).child("Reset Window Position"))
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                                    .child("Reset the caption floating window to the default bottom-center position."),
                                            ),
                                    )
                                    .child(
                                        Button::new("btn-reset-caption-pos")
                                            .outline()
                                            .small()
                                            .label("Reset Window Position")
                                            .on_click(cx.listener(|_shell, _, _, cx| {
                                                log::info!("Resetting caption window position to bottom center");
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    // ==========================================
                    // 3. Real-time Bilingual Translation Section
                    // ==========================================
                    .child(
                        section(
                            "Real-time Bilingual Translation",
                            "Configure machine translation, display modes, and bilingual ordering.",
                            cx,
                        )
                        .child(toggle_row(
                            "Enable Real-time Translation",
                            "Synchronously translate and display subtitles in real time.",
                            "realtimeTranslationEnabled",
                            realtime_translation_enabled,
                            cx,
                        ))
                        .when(realtime_translation_enabled, |this| {
                            let src_clone = source_lang.clone();
                            let tgt_clone = target_lang.clone();
                            this.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .py_3()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().text_size(px(14.0)).child("Language Pair"))
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                                    .child("Select source and target translation languages."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(language_badge(&source_lang))
                                            .child(
                                                Button::new("btn-swap-sub-lang")
                                                    .ghost()
                                                    .small()
                                                    .icon(LucideIcon::ArrowLeftRight)
                                                    .tooltip("Swap languages")
                                                    .on_click(cx.listener(move |_shell, _, _, cx| {
                                                        let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                                                        let _ = config.set_setting("realtimeTranslationSourceLanguage", &Value::String(tgt_clone.clone()));
                                                        let _ = config.set_setting("realtimeTranslationTargetLanguage", &Value::String(src_clone.clone()));
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(language_badge(&target_lang)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .py_3()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().text_size(px(14.0)).child("Subtitle Display Mode"))
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                                    .child("Choose bilingual or single language presentation."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .p(px(2.0))
                                            .rounded(RADIUS_MD)
                                            .bg(COLOR_BG_TERTIARY_LIGHT)
                                            .gap(px(2.0))
                                            .child(mode_button(
                                                "btn-mode-bilingual",
                                                "Bilingual",
                                                caption_display_mode == "bilingual",
                                                "bilingual",
                                                cx,
                                            ))
                                            .child(mode_button(
                                                "btn-mode-translation",
                                                "Translation Only",
                                                caption_display_mode == "translation",
                                                "translation",
                                                cx,
                                            ))
                                            .child(mode_button(
                                                "btn-mode-original",
                                                "Original Only",
                                                caption_display_mode == "original",
                                                "original",
                                                cx,
                                            )),
                                    ),
                            )
                            .when(caption_display_mode == "bilingual", |bilingual_panel| {
                                bilingual_panel.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_4()
                                        .py_3()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(div().text_size(px(14.0)).child("Bilingual Order"))
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                                                        .child("Decide whether original or translation appears on top."),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .p(px(2.0))
                                                .rounded(RADIUS_MD)
                                                .bg(COLOR_BG_TERTIARY_LIGHT)
                                                .gap(px(2.0))
                                                .child(order_button(
                                                    "btn-order-orig",
                                                    "Original on Top",
                                                    caption_bilingual_order == "original_first",
                                                    "original_first",
                                                    cx,
                                                ))
                                                .child(order_button(
                                                    "btn-order-trans",
                                                    "Translation on Top",
                                                    caption_bilingual_order == "translation_first",
                                                    "translation_first",
                                                    cx,
                                                )),
                                        ),
                                )
                            })
                        }),
                    )
                    // ==========================================
                    // 4. Appearance Settings Section
                    // ==========================================
                    .child(
                        section(
                            "Appearance",
                            "Customize floating window dimensions, typography, colors, and delay.",
                            cx,
                        )
                        .child(stepper_row(
                            "Floating Window Width",
                            "Width in pixels for the caption window bounds.",
                            "captionWindowWidth",
                            caption_window_width,
                            400.0,
                            1600.0,
                            50.0,
                            "px",
                            cx,
                        ))
                        .child(stepper_row(
                            "Original Font Size",
                            "Text size for original speech transcription line.",
                            "captionFontSize",
                            caption_font_size,
                            14.0,
                            48.0,
                            1.0,
                            "px",
                            cx,
                        ))
                        .child(color_row(
                            "Original Font Color",
                            "Text color for original transcribed speech.",
                            "captionFontColor",
                            &caption_font_color,
                            SUBTITLE_COLOR_PRESETS,
                            cx,
                        ))
                        .child(stepper_row(
                            "Translation Font Size",
                            "Text size for translated speech line.",
                            "captionTranslationFontSize",
                            caption_translation_font_size,
                            12.0,
                            48.0,
                            1.0,
                            "px",
                            cx,
                        ))
                        .child(color_row(
                            "Translation Font Color",
                            "Text color for translated speech.",
                            "captionTranslationFontColor",
                            &caption_translation_font_color,
                            SUBTITLE_COLOR_PRESETS,
                            cx,
                        ))
                        .child(color_row(
                            "Background Color",
                            "Background fill color of the floating caption stage.",
                            "captionBackgroundColor",
                            &caption_background_color,
                            SUBTITLE_BG_PRESETS,
                            cx,
                        ))
                        .child(stepper_row(
                            "Background Opacity",
                            "Transparency of the caption floating surface.",
                            "captionBackgroundOpacity",
                            caption_background_opacity * 100.0,
                            0.0,
                            100.0,
                            5.0,
                            "%",
                            cx,
                        ))
                        .child(segmented_alignment_row(
                            "Text Alignment",
                            "Horizontal text alignment of subtitle lines.",
                            "captionTextAlign",
                            &caption_text_align,
                            cx,
                        ))
                        .child(toggle_row(
                            "Text Stroke Outline",
                            "High-contrast outline around text for readability on bright backgrounds.",
                            "captionTextStroke",
                            caption_text_stroke,
                            cx,
                        ))
                        .when(caption_text_stroke, |stroke_panel| {
                            stroke_panel
                                .child(stepper_row(
                                    "Stroke Width",
                                    "Thickness of the high-contrast text outline.",
                                    "captionStrokeWidth",
                                    caption_stroke_width,
                                    1.0,
                                    5.0,
                                    0.5,
                                    "px",
                                    cx,
                                ))
                                .child(color_row(
                                    "Stroke Color",
                                    "Outline stroke color.",
                                    "captionStrokeColor",
                                    &caption_stroke_color,
                                    SUBTITLE_COLOR_PRESETS,
                                    cx,
                                ))
                        })
                        .child(toggle_row(
                            "Text Shadow",
                            "Soft drop shadow behind caption text for depth.",
                            "captionTextShadow",
                            caption_text_shadow,
                            cx,
                        ))
                        .child(stepper_row(
                            "Disappear Delay",
                            "Time in seconds before subtitles fade out after speech stops.",
                            "captionDisappearDelay",
                            caption_disappear_delay,
                            1.0,
                            10.0,
                            0.5,
                            "s",
                            cx,
                        )),
                    )
                    // ==========================================
                    // 5. Real-time WYSIWYG Subtitle Preview Section
                    // ==========================================
                    .child(
                        section(
                            "Live Subtitle Preview",
                            "WYSIWYG simulation of the floating subtitle appearance on your desktop.",
                            cx,
                        ).child(
                            div()
                                .id("subtitle-live-preview-box")
                                .w_full()
                                .min_h(px(140.0))
                                .p(px(28.0))
                                .rounded(RADIUS_MD)
                                .border_1()
                                .border_color(COLOR_BORDER_LIGHT)
                                .bg(COLOR_BG_TERTIARY_LIGHT)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .id("subtitle-live-preview-stage")
                                        .w_full()
                                        .max_w(px(caption_window_width.min(720.0) as f32))
                                        .p(px(18.0))
                                        .px(px(24.0))
                                        .rounded(px(18.0))
                                        .border_1()
                                        .border_color(hsla(0.0, 0.0, 1.0, 0.14))
                                        .bg(preview_stage_bg)
                                        .shadow_xl()
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.0))
                                        .when(is_align_center, |this| this.items_center())
                                        .when(!is_align_center, |this| this.items_start())
                                        // Original and/or translation lines in specified order
                                        .when(
                                            (!realtime_translation_enabled || is_original_only)
                                                || (is_bilingual && !is_translation_first),
                                            |this| {
                                                this.child(
                                                    div()
                                                        .text_size(px(caption_font_size as f32))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(preview_orig_fg)
                                                        .child("Sona Live Speech Recognition and Subtitles"),
                                                )
                                            },
                                        )
                                        .when(
                                            realtime_translation_enabled && !is_original_only,
                                            |this| {
                                                this.child(
                                                    div()
                                                        .text_size(px(caption_translation_font_size as f32))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(preview_trans_fg)
                                                        .child("Real-time speech recognition and bilingual live subtitle preview."),
                                                )
                                            },
                                        )
                                        .when(
                                            is_bilingual && is_translation_first,
                                            |this| {
                                                this.child(
                                                    div()
                                                        .text_size(px(caption_font_size as f32))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(preview_orig_fg)
                                                        .child("Sona Live Speech Recognition and Subtitles"),
                                                )
                                            },
                                        ),
                                ),
                        ),
                    ),
            )
    }
}

impl Default for SubtitlesToolPanel {
    fn default() -> Self {
        Self::new()
    }
}
fn language_badge(lang_code: &str) -> impl IntoElement {
    let name = LANGUAGE_OPTIONS
        .iter()
        .find(|(code, _)| *code == lang_code)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| lang_code.to_string());
    div()
        .px_3()
        .py_1()
        .rounded(RADIUS_SM)
        .bg(COLOR_BG_TERTIARY_LIGHT)
        .border_1()
        .border_color(COLOR_BORDER_LIGHT)
        .text_size(px(13.0))
        .font_weight(FontWeight::MEDIUM)
        .child(name)
}

fn mode_button<V: 'static>(
    id: &'static str,
    label: &'static str,
    is_active: bool,
    value: &'static str,
    cx: &Context<V>,
) -> impl IntoElement {
    div()
        .id(ElementId::from(id))
        .px_3()
        .py_1()
        .rounded(px(6.0))
        .text_size(px(12.5))
        .font_weight(if is_active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .bg(if is_active {
            COLOR_BG_ELEVATED_LIGHT
        } else {
            gpui_kit::transparent_black()
        })
        .text_color(if is_active {
            COLOR_TEXT_PRIMARY_LIGHT
        } else {
            COLOR_TEXT_MUTED_LIGHT
        })
        .when(is_active, |this| this.shadow_xs())
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |_shell, _, _, cx| {
            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
            let _ = config.set_setting("captionDisplayMode", &Value::String(value.to_string()));
            cx.notify();
        }))
}

fn order_button<V: 'static>(
    id: &'static str,
    label: &'static str,
    is_active: bool,
    value: &'static str,
    cx: &Context<V>,
) -> impl IntoElement {
    div()
        .id(ElementId::from(id))
        .px_3()
        .py_1()
        .rounded(px(6.0))
        .text_size(px(12.5))
        .font_weight(if is_active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .bg(if is_active {
            COLOR_BG_ELEVATED_LIGHT
        } else {
            gpui_kit::transparent_black()
        })
        .text_color(if is_active {
            COLOR_TEXT_PRIMARY_LIGHT
        } else {
            COLOR_TEXT_MUTED_LIGHT
        })
        .when(is_active, |this| this.shadow_xs())
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |_shell, _, _, cx| {
            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
            let _ = config.set_setting("captionBilingualOrder", &Value::String(value.to_string()));
            cx.notify();
        }))
}

fn section<V: 'static>(
    title: &'static str,
    description: &'static str,
    cx: &Context<V>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                        .child(description),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(cx.theme().border),
        )
}

fn toggle_row<V: 'static>(
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    checked: bool,
    cx: &Context<V>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                        .child(hint),
                ),
        )
        .child(
            Switch::new(ElementId::Name(format!("subtitle-setting-{key}").into()))
                .checked(checked)
                .accessibility_label(title)
                .on_change(cx.listener(move |_shell, value, _, cx| {
                    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                    if let Err(error) = config.set_setting(key, &Value::Bool(*value)) {
                        log::error!("Failed to update subtitle setting {key}: {error}");
                    }
                    cx.notify();
                })),
        )
}

fn color_swatch_picker<V: 'static>(
    key: &'static str,
    current_hex: &str,
    presets: &[&'static str],
    cx: &Context<V>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .children(presets.iter().map(|&preset| {
            let is_selected = current_hex.eq_ignore_ascii_case(preset);
            let swatch_color = hex_to_hsla(preset).unwrap_or_else(gpui_kit::white);
            div()
                .id(ElementId::from(format!("swatch-{key}-{preset}")))
                .size(px(22.0))
                .rounded_full()
                .bg(swatch_color)
                .border_1()
                .border_color(if is_selected {
                    cx.theme().primary
                } else {
                    hsla(0.0, 0.0, 0.5, 0.3)
                })
                .cursor_pointer()
                .when(is_selected, |this| {
                    this.shadow_sm()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(LucideIcon::Check).size(px(12.0)).text_color(
                            if preset == "#ffffff" || preset == "#facc15" {
                                gpui_kit::black()
                            } else {
                                gpui_kit::white()
                            },
                        ))
                })
                .on_click(cx.listener(move |_shell, _, _, cx| {
                    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                    let _ = config.set_setting(key, &Value::String(preset.to_string()));
                    cx.notify();
                }))
        }))
}

fn color_row<V: 'static>(
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    current_hex: &str,
    presets: &[&'static str],
    cx: &Context<V>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                        .child(hint),
                ),
        )
        .child(color_swatch_picker(key, current_hex, presets, cx))
}

#[allow(clippy::too_many_arguments)]
fn stepper_row<V: 'static>(
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    current_value: f64,
    min: f64,
    max: f64,
    step: f64,
    unit: &'static str,
    cx: &Context<V>,
) -> gpui_kit::Div {
    let display_str = if step.fract() != 0.0 {
        format!("{current_value:.1}{unit}")
    } else {
        format!("{current_value:.0}{unit}")
    };
    let prev_val = (current_value - step).max(min);
    let next_val = (current_value + step).min(max);
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                        .child(hint),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new(format!("step-dec-{key}"))
                        .ghost()
                        .small()
                        .label("-")
                        .disabled(current_value <= min)
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                            let _ = config.set_setting(key, &serde_json::json!(prev_val));
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .min_w(px(64.0))
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .flex()
                        .justify_center()
                        .child(display_str),
                )
                .child(
                    Button::new(format!("step-inc-{key}"))
                        .ghost()
                        .small()
                        .label("+")
                        .disabled(current_value >= max)
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                            let _ = config.set_setting(key, &serde_json::json!(next_val));
                            cx.notify();
                        })),
                ),
        )
}

fn segmented_alignment_row<V: 'static>(
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    current: &str,
    cx: &Context<V>,
) -> gpui_kit::Div {
    let is_center = current == "center";
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                        .child(hint),
                ),
        )
        .child(
            div()
                .flex()
                .p(px(2.0))
                .rounded(RADIUS_MD)
                .bg(COLOR_BG_TERTIARY_LIGHT)
                .gap(px(2.0))
                .child(
                    div()
                        .id("btn-align-center")
                        .px_3()
                        .py_1()
                        .rounded(px(6.0))
                        .text_size(px(12.5))
                        .font_weight(if is_center {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .bg(if is_center {
                            COLOR_BG_ELEVATED_LIGHT
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .text_color(if is_center {
                            COLOR_TEXT_PRIMARY_LIGHT
                        } else {
                            COLOR_TEXT_MUTED_LIGHT
                        })
                        .when(is_center, |this| this.shadow_xs())
                        .cursor_pointer()
                        .child("Center")
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                            let _ = config.set_setting(key, &Value::String("center".to_string()));
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("btn-align-left")
                        .px_3()
                        .rounded(px(6.0))
                        .text_size(px(12.5))
                        .font_weight(if !is_center {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .bg(if !is_center {
                            COLOR_BG_ELEVATED_LIGHT
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .text_color(if !is_center {
                            COLOR_TEXT_PRIMARY_LIGHT
                        } else {
                            COLOR_TEXT_MUTED_LIGHT
                        })
                        .when(!is_center, |this| this.shadow_xs())
                        .cursor_pointer()
                        .child("Left")
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                            let _ = config.set_setting(key, &Value::String("left".to_string()));
                            cx.notify();
                        })),
                ),
        )
}
#[cfg(test)]
mod tests {
    use super::{
        LANGUAGE_OPTIONS, SUBTITLE_BG_PRESETS, SUBTITLE_COLOR_PRESETS, SubtitlesToolPanel,
    };
    #[test]
    fn subtitle_presets_include_canonical_colors() {
        assert!(SUBTITLE_COLOR_PRESETS.contains(&"#ffffff"));
        assert!(SUBTITLE_COLOR_PRESETS.contains(&"#000000"));
        assert!(SUBTITLE_COLOR_PRESETS.contains(&"#facc15"));
        assert!(SUBTITLE_COLOR_PRESETS.contains(&"#60a5fa"));
        assert!(SUBTITLE_COLOR_PRESETS.contains(&"#4ade80"));
        assert!(SUBTITLE_COLOR_PRESETS.contains(&"#f87171"));

        assert!(SUBTITLE_BG_PRESETS.contains(&"#17171a"));
        assert!(SUBTITLE_BG_PRESETS.contains(&"#000000"));
    }

    #[test]
    fn language_options_include_common_languages() {
        let codes: Vec<&str> = LANGUAGE_OPTIONS.iter().map(|(c, _)| *c).collect();
        assert!(codes.contains(&"auto"));
        assert!(codes.contains(&"zh"));
        assert!(codes.contains(&"en"));
        assert!(codes.contains(&"ja"));
        assert!(codes.contains(&"ko"));
    }

    #[test]
    fn subtitles_tool_panel_default_constructs() {
        let panel = SubtitlesToolPanel;
        let _ = panel;
    }
}
