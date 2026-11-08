//! Voice Typing configuration panel.

use crate::app::GlobalDesktopServices;
use crate::views::settings::ShortcutTarget;
use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

fn persist_setting(services: &crate::services::DesktopServices, key: &str, value: Value) {
    let _ = crate::platform::agent_control::facade::settings::update_setting(
        services,
        key.to_owned(),
        value,
    );
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

fn setting_bool(services: &crate::services::DesktopServices, key: &str, default: bool) -> bool {
    services
        .config
        .get_setting(key)
        .ok()
        .flatten()
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

pub struct VoiceTypingToolPanel;

impl VoiceTypingToolPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &mut self,
        is_recording_shortcut: bool,
        shortcut_recording_target: Option<ShortcutTarget>,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let enabled = setting_bool(&services, "voiceTypingEnabled", false);
        let shortcut = setting_string(&services, "voiceTypingShortcut", "Alt+V");
        let translate_shortcut =
            setting_string(&services, "voiceTypingTranslateShortcut", "Alt+Shift+V");
        let recall_shortcut =
            setting_string(&services, "voiceTypingQuickRecallShortcut", "Alt+Shift+H");
        let mode = setting_string(&services, "voiceTypingMode", "hold");
        let placement = setting_string(&services, "voiceTypingPlacement", "caret");
        let processing = setting_string(&services, "voiceTypingProcessingMode", "raw");
        let target_language = setting_string(&services, "voiceTypingTargetLanguage", "en");
        let context_preset = setting_string(&services, "voiceTypingContextPreset", "auto");
        let sound_enabled = setting_bool(&services, "voiceTypingSoundEnabled", true);
        let cjk_spacing = setting_bool(&services, "voiceTypingCjkSpacingEnabled", true);
        let context_enabled = setting_bool(&services, "voiceTypingContextAwarenessEnabled", true);

        div()
            .id("tools-panel-voice-typing")
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
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap_4()
                            .pb_6()
                            .mb(px(28.0))
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
                                            .bg(cx.theme().group_box)
                                            .text_color(cx.theme().primary)
                                            .child(Icon::new(LucideIcon::Keyboard).size(px(24.0))),
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
                                                    .child("Voice Typing"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Configure dictation into other applications and its processing behavior."),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(if enabled { cx.theme().success } else { cx.theme().muted_foreground })
                                            .child(if enabled { "Enabled" } else { "Disabled" }),
                                    )
                                    .child(
                                        Switch::new("voice-typing-enabled")
                                            .checked(enabled)
                                            .accessibility_label("Enable Voice Typing")
                                            .on_change(cx.listener(|_this, checked, _, cx| {
                                                let services = &cx.global::<GlobalDesktopServices>().0;
                                                persist_setting(services, "voiceTypingEnabled", serde_json::json!(*checked));
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(32.0))
                            .child(section("Interaction & Shortcuts", "Configure global shortcut triggers and activation mode.", cx)
                                .child(row("Shortcut", "Global shortcut to activate voice typing.", cx)
                                    .child(
                                        Button::new("voice-typing-shortcut-capture")
                                            .outline()
                                            .small()
                                            .label(if is_recording_shortcut
                                                && shortcut_recording_target == Some(ShortcutTarget::VoiceTyping)
                                            { "Recording..." } else { shortcut.as_str() })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings_modal.is_recording_shortcut = true;
                                                this.settings_modal.shortcut_recording_target = Some(ShortcutTarget::VoiceTyping);
                                                cx.notify();
                                            })),
                                    ))
                                .child(row("Quick Recall Shortcut", "Open floating history to re-inject recent dictations.", cx)
                                    .child(
                                        Button::new("voice-typing-quick-recall-shortcut-capture")
                                            .outline()
                                            .small()
                                            .label(if is_recording_shortcut
                                                && shortcut_recording_target == Some(ShortcutTarget::VoiceTypingQuickRecall)
                                            { "Recording..." } else { recall_shortcut.as_str() })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings_modal.is_recording_shortcut = true;
                                                this.settings_modal.shortcut_recording_target = Some(ShortcutTarget::VoiceTypingQuickRecall);
                                                cx.notify();
                                            })),
                                    ))
                                .child(row("Translate Shortcut", "Dictate and translate into the configured target language.", cx)
                                    .child(
                                        Button::new("voice-typing-translate-shortcut-capture")
                                            .outline()
                                            .small()
                                            .label(if is_recording_shortcut
                                                && shortcut_recording_target == Some(ShortcutTarget::VoiceTypingTranslate)
                                            { "Recording..." } else { translate_shortcut.as_str() })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings_modal.is_recording_shortcut = true;
                                                this.settings_modal.shortcut_recording_target = Some(ShortcutTarget::VoiceTypingTranslate);
                                                cx.notify();
                                            })),
                                    ))
                                .child(row("Mode", "How the shortcut triggers listening.", cx)
                                    .child(
                                        div().flex().gap_2()
                                            .child(Button::new("voice-mode-hold").small().label("Push to Talk")
                                                .when(mode == "hold", |button| button.primary())
                                                .when(mode != "hold", |button| button.outline())
                                                .on_click(cx.listener(|_this, _, _, cx| {
                                                    let services = &cx.global::<GlobalDesktopServices>().0;
                                                    persist_setting(services, "voiceTypingMode", serde_json::json!("hold"));
                                                    cx.notify();
                                                })))
                                            .child(Button::new("voice-mode-toggle").small().label("Toggle")
                                                .when(mode == "toggle", |button| button.primary())
                                                .when(mode != "toggle", |button| button.outline())
                                                .on_click(cx.listener(|_this, _, _, cx| {
                                                    let services = &cx.global::<GlobalDesktopServices>().0;
                                                    persist_setting(services, "voiceTypingMode", serde_json::json!("toggle"));
                                                    cx.notify();
                                                }))),
                                    )))
                                .child(row("Placement Policy", "Choose where the floating capsule appears during dictation.", cx)
                                    .child(
                                        div().flex().gap_2()
                                            .child(Button::new("voice-placement-caret").small().label("Follow Caret")
                                                .when(placement == "caret", |button| button.primary())
                                                .when(placement != "caret", |button| button.outline())
                                                .on_click(cx.listener(|_this, _, _, cx| {
                                                    let services = &cx.global::<GlobalDesktopServices>().0;
                                                    persist_setting(services, "voiceTypingPlacement", serde_json::json!("caret"));
                                                    cx.notify();
                                                })))
                                            .child(Button::new("voice-placement-bottom-center").small().label("Bottom Center")
                                                .when(placement == "bottom_center", |button| button.primary())
                                                .when(placement != "bottom_center", |button| button.outline())
                                                .on_click(cx.listener(|_this, _, _, cx| {
                                                    let services = &cx.global::<GlobalDesktopServices>().0;
                                                    persist_setting(services, "voiceTypingPlacement", serde_json::json!("bottom_center"));
                                                    cx.notify();
                                                }))),
                                    )))
                            .child(section("Processing & AI Polish", "Choose between fast raw output or AI-powered smart polishing.", cx)
                                .child(row("Processing Mode", "Choose raw speech-to-text or AI-polished output.", cx)
                                    .child(
                                        div().flex().gap_2()
                                            .child(Button::new("voice-processing-raw").small().label("Fast Dictation")
                                                .when(processing == "raw", |button| button.primary())
                                                .when(processing != "raw", |button| button.outline())
                                                .on_click(cx.listener(|_this, _, _, cx| {
                                                    let services = &cx.global::<GlobalDesktopServices>().0;
                                                    persist_setting(services, "voiceTypingProcessingMode", serde_json::json!("raw"));
                                                    cx.notify();
                                                })))
                                            .child(Button::new("voice-processing-polish").small().label("Smart Polish")
                                                .when(processing == "polish", |button| button.primary())
                                                .when(processing != "polish", |button| button.outline())
                                                .on_click(cx.listener(|_this, _, _, cx| {
                                                    let services = &cx.global::<GlobalDesktopServices>().0;
                                                    persist_setting(services, "voiceTypingProcessingMode", serde_json::json!("polish"));
                                                    cx.notify();
                                                }))),
                                    )))
                             .child(row("Translation Target", "Language used by the dedicated translate shortcut.", cx)
                                 .child(
                                     div().flex().gap_2().flex_wrap()
                                         .children([
                                             ("en", "English"),
                                             ("zh", "Chinese"),
                                             ("ja", "Japanese"),
                                             ("ko", "Korean"),
                                             ("es", "Spanish"),
                                             ("fr", "French"),
                                         ].into_iter().map(|(code, label)| {
                                             let active = target_language.eq_ignore_ascii_case(code);
                                             Button::new(ElementId::from(format!("voice-target-{code}")))
                                                 .small()
                                                 .label(label)
                                                 .when(active, |button| button.primary())
                                                 .when(!active, |button| button.outline())
                                                 .on_click(cx.listener(move |_this, _, _, cx| {
                                                     let services = &cx.global::<GlobalDesktopServices>().0;
                                                     persist_setting(services, "voiceTypingTargetLanguage", serde_json::json!(code));
                                                     cx.notify();
                                                 }))
                                         })),
                                 ))
                             .child(section("Typography & Audio Feedback", "Fine-tune CJK-Latin spacing and sound cues.", cx)
                                .child(row("Audio Feedback (Earcons)", "Play sound cues on start, commit, cancel, or error.", cx)
                                    .child(Switch::new("voice-typing-sound-enabled").checked(sound_enabled)
                                        .accessibility_label("Audio Feedback (Earcons)")
                                        .on_change(cx.listener(|_this, checked, _, cx| {
                                            let services = &cx.global::<GlobalDesktopServices>().0;
                                            persist_setting(services, "voiceTypingSoundEnabled", serde_json::json!(*checked));
                                            cx.notify();
                                        })))))
                                .child(row("CJK-Latin Typography Spacing", "Automatically space mixed CJK and Latin text.", cx)
                                    .child(Switch::new("voice-typing-cjk-spacing").checked(cjk_spacing)
                                        .accessibility_label("CJK-Latin Typography Spacing")
                                        .on_change(cx.listener(|_this, checked, _, cx| {
                                            let services = &cx.global::<GlobalDesktopServices>().0;
                                            persist_setting(services, "voiceTypingCjkSpacingEnabled", serde_json::json!(*checked));
                                            cx.notify();
                                        }))))
                            .child(section("Application Context", "Adapt tone and formatting to the active application.", cx)
                                .child(row("Application Context Awareness", "Automatically adapt tone, style, and formatting based on the active application and window context.", cx)
                                    .child(Switch::new("voice-typing-context-awareness").checked(context_enabled)
                                        .accessibility_label("Application Context Awareness")
                                        .on_change(cx.listener(|_this, checked, _, cx| {
                                            let services = &cx.global::<GlobalDesktopServices>().0;
                                            persist_setting(services, "voiceTypingContextAwarenessEnabled", serde_json::json!(*checked));
                                            cx.notify();
                                         }))))
                                 .child(row("Context Preset", "Choose automatic detection or force a writing context for the next dictation.", cx)
                                     .child(
                                         div().flex().gap_2().flex_wrap()
                                             .children([
                                                 ("auto", "Auto"),
                                                 ("general", "General"),
                                                 ("developer", "Developer"),
                                                 ("chat", "Chat"),
                                                 ("formal", "Formal"),
                                             ].into_iter().map(|(preset, label)| {
                                                 let active = context_preset.eq_ignore_ascii_case(preset);
                                                 Button::new(ElementId::from(format!("voice-context-{preset}")))
                                                     .small()
                                                     .label(label)
                                                     .when(active, |button| button.primary())
                                                     .when(!active, |button| button.outline())
                                                     .on_click(cx.listener(move |_this, _, _, cx| {
                                                         let services = &cx.global::<GlobalDesktopServices>().0;
                                                         persist_setting(services, "voiceTypingContextPreset", serde_json::json!(preset));
                                                         cx.notify();
                                                     }))
                                             })),
                                     )),
                    ),
            )
    }
}

impl Default for VoiceTypingToolPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn section(
    title: &'static str,
    description: &'static str,
    cx: &Context<MainShell>,
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
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().muted_foreground)
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

fn row(title: &'static str, hint: &'static str, cx: &Context<MainShell>) -> gpui_kit::Div {
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
                .child(div().text_size(px(14.0)).child(title))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
}
