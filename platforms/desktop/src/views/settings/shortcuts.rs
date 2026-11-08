use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::SettingsModal;
use super::types::ShortcutTarget;
use crate::app::GlobalDesktopServices;

pub struct ShortcutsSettingsTab {
    pub shortcut_edit_state: Entity<InputState>,
    pub is_recording_shortcut: bool,
    pub shortcut_recording_target: Option<ShortcutTarget>,
    pub is_editing_shortcut: bool,
    pub shortcut_draft: String,
    pub live_record_shortcut: String,
    _shortcut_subscription: Subscription,
}

impl ShortcutsSettingsTab {
    pub fn new(
        initial_config: &Value,
        window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> Self {
        let live_record_shortcut = initial_config
            .get("liveRecordShortcut")
            .and_then(Value::as_str)
            .unwrap_or("F8")
            .to_string();

        let shortcut_edit_state =
            cx.new(|cx| InputState::new(window, cx).default_value(live_record_shortcut.clone()));

        let _shortcut_subscription = cx.subscribe(
            &shortcut_edit_state,
            |modal, state, event: &InputEvent, cx| {
                if !modal.shortcuts.is_editing_shortcut {
                    return;
                }
                match event {
                    InputEvent::Change => {
                        modal.shortcuts.shortcut_draft = state.read(cx).value().to_string();
                    }
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        let value = state.read(cx).value().to_string();
                        modal.shortcuts.commit_shortcut_edit(value, cx);
                    }
                    InputEvent::Focus => {}
                }
            },
        );

        Self {
            shortcut_edit_state,
            is_recording_shortcut: false,
            shortcut_recording_target: None,
            is_editing_shortcut: false,
            shortcut_draft: String::new(),
            live_record_shortcut,
            _shortcut_subscription,
        }
    }

    pub fn commit_shortcut_edit(&mut self, value: String, cx: &mut Context<SettingsModal>) {
        self.is_editing_shortcut = false;
        self.persist_shortcut(value, cx);
        cx.notify();
    }

    pub fn persist_shortcut(&mut self, shortcut: String, cx: &mut Context<SettingsModal>) {
        self.live_record_shortcut = shortcut.clone();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = crate::platform::agent_control::facade::settings::update_setting(
                &services,
                "liveRecordShortcut".to_string(),
                Value::String(shortcut),
            );
        })
        .detach();
    }

    pub fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> impl IntoElement {
        let is_editing = self.is_editing_shortcut;
        let is_recording = self.is_recording_shortcut;
        let live_record = self.live_record_shortcut.clone();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Live Recording"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .py_1()
                            .child(div().text_size(px(13.0)).child("Start / Stop Recording"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .when(is_editing, |this| {
                                        this.child(
                                            div()
                                                .w(px(120.0))
                                                .h(px(28.0))
                                                .px_2()
                                                .py_1()
                                                .text_center()
                                                .rounded_sm()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .bg(cx.theme().input)
                                                .text_size(px(14.0))
                                                .text_color(cx.theme().foreground)
                                                .child(Input::new(&self.shortcut_edit_state)),
                                        )
                                    })
                                    .when(!is_editing, |this| {
                                        this.child(
                                            Button::new("shortcut-live-record-capture")
                                                .outline()
                                                .small()
                                                .label(if is_recording {
                                                    "Recording..."
                                                } else {
                                                    live_record.as_str()
                                                })
                                                .on_click(cx.listener(|modal, _, _, cx| {
                                                    modal.shortcuts.is_recording_shortcut = true;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("shortcut-live-record-edit")
                                                .ghost()
                                                .small()
                                                .label("Edit manually")
                                                .on_click(cx.listener(|modal, _, window, cx| {
                                                    let value = modal
                                                        .shortcuts
                                                        .live_record_shortcut
                                                        .clone();
                                                    modal.shortcuts.shortcut_draft = value.clone();
                                                    modal.shortcuts.is_editing_shortcut = true;
                                                    modal.shortcuts.shortcut_edit_state.update(
                                                        cx,
                                                        |state, cx| {
                                                            state.set_value(value, window, cx);
                                                            state.focus(window, cx);
                                                        },
                                                    );
                                                    cx.notify();
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(shortcut_section(
                        "",
                        vec![("Pause / Resume Recording", "Space")],
                        cx,
                    )),
            )
            .child(shortcut_section(
                "Playback",
                vec![
                    ("Play / Pause", "Space / K"),
                    ("Seek Backward 5s / Seek Forward 5s", "← / →"),
                    ("Volume Up / Volume Down", "↑ / ↓"),
                    ("Toggle Mute", "M"),
                ],
                cx,
            ))
            .child(shortcut_section(
                "Search",
                vec![
                    ("Find in current context", "Ctrl + F"),
                    ("Next / previous transcript match", "Enter / Shift + Enter"),
                    ("Navigate workspace search results", "↑ / ↓"),
                    ("Clear or close current search", "Esc"),
                ],
                cx,
            ))
            .child(shortcut_section(
                "Editor",
                vec![
                    ("Bold", "Ctrl + B"),
                    ("Italic", "Ctrl + I"),
                    ("Underline", "Ctrl + U"),
                    ("Strikethrough", "Ctrl + Shift + S"),
                    ("Split Segment", "Shift + Enter"),
                ],
                cx,
            ))
    }
}

pub(crate) fn shortcut_section(
    title: &'static str,
    shortcuts: Vec<(&'static str, &str)>,
    cx: &Context<SettingsModal>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .when(!title.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
        })
        .children(shortcuts.into_iter().map(|(description, keys)| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().foreground)
                        .child(description),
                )
                .child(div().flex().items_center().gap_1().children(
                    keys.split(" / ").enumerate().flat_map(|(index, key)| {
                        let mut elements = vec![
                            div()
                                .min_w(px(24.0))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().input)
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_center()
                                .child(key.to_string())
                                .into_any_element(),
                        ];
                        if index + 1 < keys.split(" / ").count() {
                            elements.push(
                                div()
                                    .mx_1()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child("/")
                                    .into_any_element(),
                            );
                        }
                        elements
                    }),
                ))
        }))
}
