use gpui_kit::component::IndexPath;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::SettingsModal;
use super::languages::LANGUAGE_OPTIONS;
use crate::views::tools::{SubtitlesToolPanel, VoiceTypingToolPanel};

pub struct SubtitlesSettingsTab {
    pub subtitle_voice_typing_tab: bool,
    pub subtitles_tool: SubtitlesToolPanel,
    pub voice_typing_tool: VoiceTypingToolPanel,
    pub translation_language_select: Entity<SelectState<Vec<&'static str>>>,
    _translation_language_subscription: Subscription,
}

impl SubtitlesSettingsTab {
    pub fn new(
        initial_config: &Value,
        window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> Self {
        let initial_lang = initial_config
            .get("realtimeTranslationTargetLanguage")
            .and_then(Value::as_str)
            .unwrap_or("en");

        let lang_index = LANGUAGE_OPTIONS
            .iter()
            .position(|(code, _)| *code == initial_lang)
            .unwrap_or(0);

        let translation_language_select = cx.new(|cx| {
            SelectState::new(
                LANGUAGE_OPTIONS
                    .iter()
                    .map(|(_, label)| *label)
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(lang_index)),
                window,
                cx,
            )
        });

        let _translation_language_subscription = cx.subscribe(
            &translation_language_select,
            |_modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event
                    && let Some((code, _)) =
                        LANGUAGE_OPTIONS.iter().find(|(_, label)| label == value)
                {
                    let code_owned = (*code).to_string();
                    let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
                    cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                        let _ = services.config.set_setting(
                            "realtimeTranslationTargetLanguage",
                            &Value::String(code_owned),
                        );
                    })
                    .detach();
                    cx.notify();
                }
            },
        );

        Self {
            subtitle_voice_typing_tab: false,
            subtitles_tool: SubtitlesToolPanel::new(),
            voice_typing_tool: VoiceTypingToolPanel::new(),
            translation_language_select,
            _translation_language_subscription,
        }
    }

    pub fn render(
        &mut self,
        is_rec: bool,
        target: Option<super::types::ShortcutTarget>,
        cx: &mut Context<SettingsModal>,
    ) -> impl IntoElement {
        let is_voice_typing = self.subtitle_voice_typing_tab;

        div()
            .flex()
            .flex_col()
            .gap_4()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .px(px(32.0))
                    .pt(px(20.0))
                    .child(
                        Button::new("settings-subtitle-voice-typing")
                            .when(is_voice_typing, |button| button.primary())
                            .when(!is_voice_typing, |button| button.outline())
                            .small()
                            .label("Voice Typing")
                            .on_click(cx.listener(|modal, _, _, cx| {
                                modal.subtitles.subtitle_voice_typing_tab = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("settings-subtitle-caption-settings")
                            .when(!is_voice_typing, |button| button.primary())
                            .when(is_voice_typing, |button| button.outline())
                            .small()
                            .label("Subtitle Settings")
                            .on_click(cx.listener(|modal, _, _, cx| {
                                modal.subtitles.subtitle_voice_typing_tab = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().child(if is_voice_typing {
                self.voice_typing_tool
                    .render(is_rec, target, cx, |modal, target, _cx| {
                        modal.shortcuts.is_recording_shortcut = true;
                        modal.shortcuts.shortcut_recording_target = Some(target);
                    })
                    .into_any_element()
            } else {
                self.subtitles_tool.render(cx).into_any_element()
            }))
    }
}
