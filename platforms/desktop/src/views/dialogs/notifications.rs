//! Complete Notification and Task Center flyout popover panel.
//! Replaces mock strings with live task_ledger records grouped into 3 sections.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::task_ledger::types::{TaskLedgerRecord, TaskLedgerStatus};

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{COLOR_PROJECT_AMBER, RADIUS_LG, RADIUS_MD, RADIUS_SM};

pub struct NotificationCenter {
    pub is_open: bool,
    pub tasks: Vec<TaskLedgerRecord>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            is_open: false,
            tasks: Vec::new(),
        }
    }

    pub fn load_tasks(&mut self, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let tasks = services.task_ledger.list_tasks().await.unwrap_or_default();
            this.update(cx, |view, cx| {
                view.tasks = tasks;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn clear_resolved(&mut self, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let _ = services.task_ledger.clear_resolved().await;
            let tasks = services.task_ledger.list_tasks().await.unwrap_or_default();
            this.update(cx, |view, cx| {
                view.tasks = tasks;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.is_open = !self.is_open;
        if self.is_open {
            self.load_tasks(cx);
        }
        cx.notify();
    }

    pub fn set_tasks(&mut self, tasks: Vec<TaskLedgerRecord>, cx: &mut Context<Self>) {
        self.tasks = tasks;
        cx.notify();
    }

    pub fn badge_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| {
                !t.status.is_terminal()
                    || matches!(
                        t.status,
                        TaskLedgerStatus::Failed | TaskLedgerStatus::Recoverable
                    )
            })
            .count()
    }

    pub fn badge_label(&self) -> String {
        let count = self.badge_count();
        if count > 99 {
            "99+".to_string()
        } else {
            count.to_string()
        }
    }

    pub fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let badge_count = self.badge_count();
        let badge_text = self.badge_label();

        let action_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskLedgerStatus::Failed
                        | TaskLedgerStatus::Recoverable
                        | TaskLedgerStatus::Interrupted
                )
            })
            .collect();

        let active_tasks: Vec<_> = self.tasks.iter().filter(|t| t.status.is_active()).collect();

        let info_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskLedgerStatus::Succeeded | TaskLedgerStatus::Cancelled
                )
            })
            .collect();

        div()
            .relative()
            .child(
                // Trigger Button: 36x36px icon button with Bell icon and Red Badge
                div()
                    .id("notification-center-trigger")
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.0))
                    .rounded(RADIUS_MD)
                    .bg(if self.is_open {
                        cx.theme().secondary
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle(cx);
                    }))
                    .child(Icon::new(IconName::Bell).size(px(18.0)))
                    .when(badge_count > 0, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(2.0))
                                .right(px(2.0))
                                .min_w(px(14.0))
                                .h(px(14.0))
                                .px(px(3.0))
                                .rounded_full()
                                .bg(cx.theme().primary)
                                .text_color(gpui_kit::white())
                                .text_size(px(9.0))
                                .font_weight(FontWeight::BOLD)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(badge_text),
                        )
                    }),
            )
            // Flyout Popover Panel (width 380px, max-height 540px, padding 16px)
            .when(self.is_open, |this| {
                this.child(
                    div()
                        .id("notification-center-panel")
                        .absolute()
                        .top(px(42.0))
                        .right_0()
                        .w(px(380.0))
                        .max_h(px(540.0))
                        .p_4()
                        .rounded(RADIUS_LG)
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .overflow_scroll()
                        // Panel Header
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(15.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Task Center"),
                                )
                                .child(
                                    Button::new("btn-clear-resolved")
                                        .ghost()
                                        .xsmall()
                                        .label("Clear Succeeded")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.clear_resolved(cx);
                                        })),
                                ),
                        )
                        // Section 1: Needs Action
                        .when(!action_tasks.is_empty(), |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(COLOR_PROJECT_AMBER)
                                            .child("Needs action"),
                                    )
                                    .children(action_tasks.into_iter().enumerate().map(
                                        |(i, task)| {
                                            div()
                                                .id(ElementId::from(format!("notif-action-{}", i)))
                                                .p_2p5()
                                                .rounded(RADIUS_SM)
                                                .bg(cx.theme().input)
                                                .border_1()
                                                .border_color(COLOR_PROJECT_AMBER.opacity(0.3))
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_size(px(13.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(task.title.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(cx.theme().muted)
                                                        .child(format!(
                                                            "{:?} - retry available",
                                                            task.kind
                                                        )),
                                                )
                                        },
                                    )),
                            )
                        })
                        // Section 2: Active (in-progress)
                        .when(!active_tasks.is_empty(), |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().primary)
                                            .child("Active"),
                                    )
                                    .children(active_tasks.into_iter().enumerate().map(
                                        |(i, task)| {
                                            let progress = if task.progress.is_finite() {
                                                (task.progress as f32).clamp(0.0, 1.0)
                                            } else {
                                                0.0
                                            };
                                            div()
                                                .id(ElementId::from(format!("notif-active-{}", i)))
                                                .p_2p5()
                                                .rounded(RADIUS_SM)
                                                .bg(cx.theme().input)
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                                .text_size(px(13.0))
                                                                .child(task.title.clone()),
                                                        )
                                                        .child(div().text_size(px(11.0)).child(
                                                            format!("{:.0}%", progress * 100.0),
                                                        )),
                                                )
                                                // Progress bar track and fill
                                                .child(
                                                    div()
                                                        .h(px(4.0))
                                                        .w_full()
                                                        .rounded_full()
                                                        .bg(cx.theme().secondary)
                                                        .child(
                                                            div()
                                                                .h_full()
                                                                .w(relative(progress))
                                                                .rounded_full()
                                                                .bg(cx.theme().primary),
                                                        ),
                                                )
                                        },
                                    )),
                            )
                        })
                        // Section 3: Recent (completed)
                        .when(!info_tasks.is_empty(), |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().muted)
                                            .child("Recent"),
                                    )
                                    .children(info_tasks.into_iter().take(5).enumerate().map(
                                        |(i, task)| {
                                            div()
                                                .id(ElementId::from(format!("notif-info-{}", i)))
                                                .p_2()
                                                .rounded(RADIUS_SM)
                                                .bg(cx.theme().input)
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .child(task.title.clone()),
                                                )
                                                .child(
                                                    Icon::new(IconName::Check)
                                                        .size(px(12.0))
                                                        .text_color(cx.theme().success),
                                                )
                                        },
                                    )),
                            )
                        })
                        // Empty state
                        .when(self.tasks.is_empty(), |this| {
                            this.child(
                                div()
                                    .py_8()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(13.0))
                                    .text_color(cx.theme().muted)
                                    .child("No active tasks right now."),
                            )
                        }),
                )
            })
    }
}

impl Render for NotificationCenter {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_content(cx)
    }
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use core::prelude::v1::test;

    #[test]
    fn test_task_progress_clamping_logic() {
        let clamp_progress = |progress: f64| -> f32 {
            if progress.is_finite() {
                (progress as f32).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };

        // Standard progress values
        assert_eq!(clamp_progress(0.0), 0.0);
        assert_eq!(clamp_progress(0.5), 0.5);
        assert_eq!(clamp_progress(1.0), 1.0);

        // Out-of-bounds progress values
        assert_eq!(clamp_progress(-0.25), 0.0);
        assert_eq!(clamp_progress(1.25), 1.0);

        // Non-finite values
        assert_eq!(clamp_progress(f64::NAN), 0.0);
        assert_eq!(clamp_progress(f64::INFINITY), 0.0);
        assert_eq!(clamp_progress(f64::NEG_INFINITY), 0.0);
    }
}
