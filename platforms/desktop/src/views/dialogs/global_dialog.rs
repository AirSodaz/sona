//! Global dialog component for alerts, confirmations, and prompts.
//! Strict 1:1 parity with React `GlobalDialog.tsx`.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::tokens::{
    COLOR_PROJECT_AMBER, COLOR_PROJECT_BLUE, RADIUS_LG, RADIUS_MD, RADIUS_SM,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Alert,
    Confirm,
    Prompt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogVariant {
    Info,
    Warning,
    Error,
    Success,
}
#[derive(Clone, Debug)]
pub enum GlobalDialogEvent {
    Confirmed {
        title: String,
        checkbox_checked: bool,
    },
    Cancelled {
        title: String,
    },
}

impl EventEmitter<GlobalDialogEvent> for GlobalDialog {}

pub struct GlobalDialog {
    pub is_open: bool,
    pub title: String,
    pub message: String,
    pub kind: DialogKind,
    pub variant: DialogVariant,
    pub checkbox_label: Option<String>,
    pub checkbox_checked: bool,
    pub checkbox_notice: Option<String>,
}

impl GlobalDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            title: String::new(),
            message: String::new(),
            kind: DialogKind::Alert,
            variant: DialogVariant::Info,
            checkbox_label: None,
            checkbox_checked: false,
            checkbox_notice: None,
        }
    }

    pub fn show(&mut self, title: impl Into<String>, msg: impl Into<String>, kind: DialogKind) {
        self.title = title.into();
        self.message = msg.into();
        self.kind = kind;
        self.variant = match kind {
            DialogKind::Confirm => DialogVariant::Warning,
            _ => DialogVariant::Info,
        };
        self.checkbox_label = None;
        self.checkbox_checked = false;
        self.checkbox_notice = None;
        self.is_open = true;
    }

    pub fn show_with_variant(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        kind: DialogKind,
        variant: DialogVariant,
    ) {
        self.title = title.into();
        self.message = msg.into();
        self.kind = kind;
        self.variant = variant;
        self.checkbox_label = None;
        self.checkbox_checked = false;
        self.checkbox_notice = None;
        self.is_open = true;
    }

    pub fn set_checkbox(&mut self, label: impl Into<String>, notice: Option<String>) {
        self.checkbox_label = Some(label.into());
        self.checkbox_checked = false;
        self.checkbox_notice = notice;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.checkbox_label = None;
        self.checkbox_checked = false;
        self.checkbox_notice = None;
    }
}

impl Render for GlobalDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_open {
            return div();
        }
        let kind = self.kind;
        let title = self.title.clone();
        div().when(self.is_open, |this| {
            this.child(
                div()
                    .id("global-dialog-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("global-dialog-content")
                            .flex()
                            .flex_col()
                            .w(px(440.0))
                            .p_6()
                            .rounded(RADIUS_LG)
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_2xl()
                            .gap_4()
                            .on_click(|_, _, cx| cx.stop_propagation())
                            // Header: Variant icon + Title
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(match self.variant {
                                        DialogVariant::Error => Icon::new(LucideIcon::CircleAlert)
                                            .size(px(22.0))
                                            .text_color(hsla(0.0, 0.84, 0.60, 1.0)),
                                        DialogVariant::Warning => {
                                            Icon::new(LucideIcon::TriangleAlert)
                                                .size(px(22.0))
                                                .text_color(COLOR_PROJECT_AMBER)
                                        }
                                        DialogVariant::Success => {
                                            Icon::new(LucideIcon::CircleCheck)
                                                .size(px(22.0))
                                                .text_color(cx.theme().success)
                                        }
                                        DialogVariant::Info => Icon::new(LucideIcon::Info)
                                            .size(px(22.0))
                                            .text_color(COLOR_PROJECT_BLUE),
                                    })
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(self.title.clone()),
                                    ),
                            )
                            // Message
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .line_height(relative(1.5))
                                    .text_color(cx.theme().muted)
                                    .child(self.message.clone()),
                            )
                            // Optional Checkbox
                            .when_some(self.checkbox_label.clone(), |this, label| {
                                let is_checked = self.checkbox_checked;
                                let notice = self.checkbox_notice.clone();
                                this.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .p_2p5()
                                        .rounded(RADIUS_MD)
                                        .bg(cx.theme().secondary)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            div()
                                                .id("global-dialog-checkbox-toggle")
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.checkbox_checked = !this.checkbox_checked;
                                                    cx.notify();
                                                }))
                                                .child(
                                                    div()
                                                        .size(px(16.0))
                                                        .rounded(RADIUS_SM)
                                                        .border_1()
                                                        .border_color(if is_checked {
                                                            cx.theme().primary
                                                        } else {
                                                            cx.theme().border
                                                        })
                                                        .bg(if is_checked {
                                                            cx.theme().primary
                                                        } else {
                                                            cx.theme().background
                                                        })
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .when(is_checked, |ch| {
                                                            ch.child(
                                                                Icon::new(LucideIcon::Check)
                                                                    .size(px(12.0))
                                                                    .text_color(
                                                                        cx.theme().background,
                                                                    ),
                                                            )
                                                        }),
                                                )
                                                .child(div().text_size(px(13.0)).child(label)),
                                        )
                                        .when(is_checked && notice.is_some(), |this| {
                                            this.child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .pl_6()
                                                    .text_size(px(11.0))
                                                    .text_color(hsla(0.0, 0.84, 0.60, 1.0))
                                                    .child(
                                                        Icon::new(LucideIcon::TriangleAlert)
                                                            .size(px(12.0)),
                                                    )
                                                    .child(notice.unwrap_or_default()),
                                            )
                                        }),
                                )
                            })
                            // Actions
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
                                                        let title = this.title.clone();
                                                        this.close();
                                                        cx.emit(GlobalDialogEvent::Cancelled {
                                                            title,
                                                        });
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
                                                } else if matches!(
                                                    title.as_str(),
                                                    "Change Data Directory?"
                                                        | "Change Models Directory?"
                                                ) {
                                                    "Confirm & Migrate"
                                                } else if matches!(
                                                    title.as_str(),
                                                    "Restore Default Data Directory?"
                                                        | "Restore Default Models Directory?"
                                                ) {
                                                    "Restore Default"
                                                } else {
                                                    "Download"
                                                }
                                            } else {
                                                "OK"
                                            })
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                let title = this.title.clone();
                                                let checkbox_checked = this.checkbox_checked;
                                                this.close();
                                                cx.emit(GlobalDialogEvent::Confirmed {
                                                    title,
                                                    checkbox_checked,
                                                });
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
        })
    }
}

impl Default for GlobalDialog {
    fn default() -> Self {
        Self::new()
    }
}
