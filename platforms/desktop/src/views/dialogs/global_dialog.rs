//! Global dialog component for alerts, confirmations, and prompts.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::shell::MainShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Alert,
    Confirm,
    Prompt,
}

pub struct GlobalDialog {
    pub is_open: bool,
    pub title: String,
    pub message: String,
    pub kind: DialogKind,
}

impl GlobalDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            title: String::new(),
            message: String::new(),
            kind: DialogKind::Alert,
        }
    }

    pub fn show(&mut self, title: impl Into<String>, msg: impl Into<String>, kind: DialogKind) {
        self.title = title.into();
        self.message = msg.into();
        self.kind = kind;
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let kind = self.kind;
        let title = self.title.clone();
        div().when(self.is_open, |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(420.0))
                            .p_6()
                            .rounded_xl()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_xl()
                            .gap_4()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(self.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(cx.theme().muted)
                                    .child(self.message.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap_2()
                                    .when(
                                        kind == DialogKind::Confirm || kind == DialogKind::Prompt,
                                        |actions| {
                                            actions.child(
                                                Button::new("btn-dialog-cancel")
                                                    .ghost()
                                                    .small()
                                                    .label("Cancel")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if this.global_dialog.title
                                                            == "Delete LLM Provider"
                                                        {
                                                            this.settings_modal
                                                                .llm_pending_delete_provider = None;
                                                        }
                                                        if this.global_dialog.title == "Regenerate API Key" {
                                                            this.settings_modal.api_server_pending_key = None;
                                                        }
                                                        if this.global_dialog.title == "Change Data Directory?"
                                                            || this.global_dialog.title == "Change Models Directory?"
                                                        {
                                                            this.settings_modal.storage_pending_migration = None;
                                                        }
                                                        if this.global_dialog.title == "Clean History Audio?" {
                                                            this.settings_modal.storage_cleanup_pending = None;
                                                        }
                                                        if matches!(this.global_dialog.title.as_str(), "Restore Default Data Directory?" | "Restore Default Models Directory?") {
                                                            this.settings_modal.storage_pending_reset = None;
                                                        }
                                                        this.global_dialog.close();
                                                        cx.notify();
                                                    })),
                                            )
                                        },
                                    )
                                    .child(
                                        Button::new("btn-dialog-ok")
                                            .primary()
                                            .small()
                                                    .label(if kind == DialogKind::Confirm {
                                                if title == "Delete LLM Provider" {
                                                    "Delete"
                                                } else if title == "Regenerate API Key" {
                                                    "Generate New Key"
                                                } else if title == "Clean History Audio?" {
                                                    "Clean Now"
                                                } else if matches!(title.as_str(), "Change Data Directory?" | "Change Models Directory?") {
                                                    "Confirm & Migrate"
                                                } else if matches!(title.as_str(), "Restore Default Data Directory?" | "Restore Default Models Directory?") {
                                                    "Restore Default"
                                                } else {
                                                    "Download"
                                                }
                                            } else {
                                                "OK"
                                            })
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let confirm_cuda = this.global_dialog.kind
                                                    == DialogKind::Confirm
                                                    && this.global_dialog.title
                                                        == "Download CUDA Add-on";
                                                let confirm_provider_delete =
                                                    this.global_dialog.kind == DialogKind::Confirm
                                                        && this.global_dialog.title
                                                            == "Delete LLM Provider";
                                                let confirm_api_key_regeneration =
                                                    this.global_dialog.kind == DialogKind::Confirm
                                                        && this.global_dialog.title == "Regenerate API Key";
                                                let confirm_storage_migration =
                                                    this.global_dialog.kind == DialogKind::Confirm
                                                        && matches!(this.global_dialog.title.as_str(), "Change Data Directory?" | "Change Models Directory?")
                                                        && this.settings_modal.storage_pending_migration.is_some();
                                                let confirm_storage_cleanup =
                                                    this.global_dialog.kind == DialogKind::Confirm
                                                        && this.global_dialog.title == "Clean History Audio?"
                                                        && this.settings_modal.storage_cleanup_pending.is_some();
                                                let confirm_storage_reset =
                                                    this.global_dialog.kind == DialogKind::Confirm
                                                        && matches!(this.global_dialog.title.as_str(), "Restore Default Data Directory?" | "Restore Default Models Directory?")
                                                        && this.settings_modal.storage_pending_reset.is_some();
                                                this.global_dialog.close();
                                                if confirm_cuda {
                                                    this.settings_modal
                                                        .begin_cuda_addon_download(cx);
                                                } else if confirm_provider_delete {
                                                    this.settings_modal
                                                        .confirm_delete_llm_custom_provider(cx);
                                                } else if confirm_api_key_regeneration {
                                                    if let Some(key) = this.settings_modal.api_server_pending_key.take() {
                                                        if let Err(error) = cx
                                                            .global::<crate::app::GlobalDesktopServices>()
                                                            .0
                                                            .config
                                                            .set_setting("httpServerApiKey", &serde_json::json!(key.clone()))
                                                        {
                                                            log::error!("Failed to regenerate API key: {error}");
                                                    } else if let Some(state) = this.api_server_field_states.get("apiKey") {
                                                            state.update(cx, |state, cx| state.set_value(key, window, cx));
                                                        }
                                                    }
                                                    cx.notify();
                                                } else if confirm_storage_migration {
                                                    this.settings_modal.confirm_storage_migration(cx);
                                                } else if confirm_storage_cleanup {
                                                    this.settings_modal.confirm_storage_cleanup(cx);
                                                } else if confirm_storage_reset {
                                                    this.settings_modal.confirm_storage_directory_reset(cx);
                                                } else {
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
        })
    }
}
