//! Settings Modal with 9 major configuration slices.

use crate::theme::tokens::*;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SettingsTab {
    General,
    Shortcuts,
    Audio,
    Models,
    Vocabulary,
    Intelligence,
    Automation,
    Storage,
    Sync,
}

pub struct SettingsModal {
    pub is_open: bool,
    pub current_tab: SettingsTab,
    pub autostart: bool,
    pub minimize_to_tray: bool,
    pub selected_theme: String,
    pub selected_lang: String,
    pub selected_mic: String,
}

impl SettingsModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            current_tab: SettingsTab::General,
            autostart: true,
            minimize_to_tray: true,
            selected_theme: "Dark (System)".to_string(),
            selected_lang: "English (US)".to_string(),
            selected_mic: "Default Audio Device".to_string(),
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn set_tab(&mut self, tab: SettingsTab) {
        self.current_tab = tab;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        let tabs = [
            (SettingsTab::General, "General"),
            (SettingsTab::Shortcuts, "Shortcuts"),
            (SettingsTab::Audio, "Audio & Mic"),
            (SettingsTab::Models, "Models"),
            (SettingsTab::Vocabulary, "Vocabulary"),
            (SettingsTab::Intelligence, "Intelligence & LLM"),
            (SettingsTab::Automation, "Automation"),
            (SettingsTab::Storage, "Storage"),
            (SettingsTab::Sync, "Cloud Sync"),
        ];

        div()
            .when(self.is_open, |this| {
                this.child(
                    // Modal backdrop overlay
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .child(
                            // Modal card
                            div()
                                .flex()
                                .w(SETTINGS_MODAL_WIDTH)
                                .h(SETTINGS_MODAL_HEIGHT)
                                .rounded_xl()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_xl()
                                .child(
                                    // Left tabs sidebar
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(px(200.0))
                                        .h_full()
                                        .p_3()
                                        .border_r_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().background)
                                        .gap_1()
                                        .child(
                                            div()
                                                .px_2()
                                                .py_1()
                                                .text_size(px(14.0))
                                                .font_weight(FontWeight::BOLD)
                                                .child("Settings"),
                                        )
                                        .children(tabs.into_iter().map(|(tab, label)| {
                                            let is_selected = self.current_tab == tab;
                                            div()
                                                .id(ElementId::from(format!("tab-btn-{:?}", tab)))
                                                .flex()
                                                .items_center()
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(if is_selected {
                                                    cx.theme().accent
                                                } else {
                                                    gpui_kit::transparent_black()
                                                })
                                                .cursor_pointer()
                                                .child(
                                                    div()
                                                        .text_size(px(13.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(if is_selected {
                                                            cx.theme().primary
                                                        } else {
                                                            cx.theme().foreground
                                                        })
                                                        .child(label),
                                                )
                                        })),
                                )
                                .child(
                                    // Right content area
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .h_full()
                                        .p_6()
                                        .gap_4()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_size(px(18.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(format!("{:?} Settings", self.current_tab)),
                                                )
                                                .child(
                                                    Button::new("btn-close-settings-modal")
                                                        .ghost()
                                                        .small()
                                                        .label("✕")
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .text_size(px(13.0))
                                                        .text_color(cx.theme().muted)
                                                        .child("Manage all application options, speech recognition backends, and system parameters."),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .p_3()
                                                        .rounded_md()
                                                        .bg(cx.theme().input)
                                                        .child("Launch at Windows startup")
                                                        .child(Button::new("btn-toggle-autostart").outline().small().label("Enabled")),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .p_3()
                                                        .rounded_md()
                                                        .bg(cx.theme().input)
                                                        .child("Minimize to system tray on close")
                                                        .child(Button::new("btn-toggle-tray").outline().small().label("Enabled")),
                                                ),
                                        ),
                                ),
                        ),
                )
            })
    }
}
