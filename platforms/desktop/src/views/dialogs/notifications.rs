//! Complete Notification and Task Center flyout popover panel.
//! Replaces mock strings with live task_ledger records grouped into 3 sections.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::task_ledger::types::{TaskLedgerRecord, TaskLedgerStatus};

use crate::app::{GlobalDesktopServices, GlobalEventBus};
use crate::theme::tokens::{COLOR_PROJECT_AMBER, RADIUS_LG, RADIUS_MD, RADIUS_SM};

fn is_actionable(task: &TaskLedgerRecord) -> bool {
    matches!(
        task.status,
        TaskLedgerStatus::Failed | TaskLedgerStatus::Recoverable | TaskLedgerStatus::Interrupted
    )
}

fn progress_fraction(progress: f64) -> f32 {
    if progress.is_finite() {
        (progress as f32 / 100.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn badge_label_for_count(count: usize) -> String {
    if count > 9 {
        "9+".to_string()
    } else {
        count.to_string()
    }
}

fn task_body(task: &TaskLedgerRecord) -> String {
    let kind = match task.kind {
        sona_core::task_ledger::types::TaskLedgerKind::BatchImport => "Batch import",
        sona_core::task_ledger::types::TaskLedgerKind::Automation => "Automation",
        sona_core::task_ledger::types::TaskLedgerKind::LlmPolish => "LLM polish",
        sona_core::task_ledger::types::TaskLedgerKind::LlmTranslate => "Translation",
        sona_core::task_ledger::types::TaskLedgerKind::LlmSummary => "AI summary",
        sona_core::task_ledger::types::TaskLedgerKind::Recovery => "Recovery",
        sona_core::task_ledger::types::TaskLedgerKind::Update => "Update",
    };
    let status = match task.status {
        TaskLedgerStatus::Pending => "Pending",
        TaskLedgerStatus::Running => "Running",
        TaskLedgerStatus::CancelRequested => "Stopping",
        TaskLedgerStatus::Failed => "Failed",
        TaskLedgerStatus::Recoverable => "Recoverable",
        TaskLedgerStatus::Interrupted => "Interrupted",
        TaskLedgerStatus::Cancelled => "Cancelled",
        TaskLedgerStatus::Succeeded => "Succeeded",
    };
    let stage = task
        .stage
        .as_deref()
        .map(|stage| format!(" · {stage}"))
        .unwrap_or_default();
    let file = task
        .file_path
        .as_deref()
        .and_then(|path| path.rsplit(['/', '\\']).next());
    match file.filter(|name| !name.is_empty() && *name != task.title) {
        Some(file) => format!("{kind} · {status}{stage} · {file}"),
        None => format!("{kind} · {status}{stage}"),
    }
}

fn task_detail(task: &TaskLedgerRecord) -> String {
    task.error_message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| task.stage.clone().unwrap_or_default())
}

pub struct NotificationCenter {
    pub is_open: bool,
    pub clear_menu_open: bool,
    pub tasks: Vec<TaskLedgerRecord>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            is_open: false,
            clear_menu_open: false,
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

    pub fn clear_tasks(&mut self, succeeded_only: bool, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            if let Ok(tasks) = services.task_ledger.list_tasks().await {
                for task in tasks.into_iter().filter(|task| {
                    !task.status.is_active()
                        && (!succeeded_only || task.status == TaskLedgerStatus::Succeeded)
                }) {
                    let _ = services.task_ledger.remove_task(task.id).await;
                }
            }
            let tasks = services.task_ledger.list_tasks().await.unwrap_or_default();
            this.update(cx, |view, cx| {
                view.tasks = tasks;
                view.clear_menu_open = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.is_open = !self.is_open;
        self.clear_menu_open = false;
        if self.is_open {
            self.load_tasks(cx);
        }
        cx.notify();
    }

    pub fn set_tasks(&mut self, tasks: Vec<TaskLedgerRecord>, cx: &mut Context<Self>) {
        self.tasks = tasks;
        cx.notify();
    }

    fn dismiss(&mut self, id: String, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let _ = services.task_ledger.remove_task(id).await;
            let tasks = services.task_ledger.list_tasks().await.unwrap_or_default();
            this.update(cx, |view, cx| {
                view.tasks = tasks;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn badge_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| {
                t.status.is_active()
                    || matches!(
                        t.status,
                        TaskLedgerStatus::Failed
                            | TaskLedgerStatus::Recoverable
                            | TaskLedgerStatus::Interrupted
                    )
            })
            .count()
    }

    pub fn badge_label(&self) -> String {
        badge_label_for_count(self.badge_count())
    }

    pub fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let badge_count = self.badge_count();
        let badge_text = self.badge_label();
        let succeeded_count = self
            .tasks
            .iter()
            .filter(|task| task.status == TaskLedgerStatus::Succeeded)
            .count();
        let clearable_count = self
            .tasks
            .iter()
            .filter(|task| !task.status.is_active())
            .count();
        let mut sorted_tasks = self.tasks.clone();
        sorted_tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let action_tasks: Vec<_> = sorted_tasks.iter().filter(|t| is_actionable(t)).collect();

        let active_tasks: Vec<_> = sorted_tasks
            .iter()
            .filter(|t| t.status.is_active())
            .collect();

        let info_tasks: Vec<_> = sorted_tasks
            .iter()
            .filter(|t| !is_actionable(t) && !t.status.is_active())
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
                                .top(px(1.0))
                                .right(px(1.0))
                                .min_w(px(14.0))
                                .h(px(14.0))
                                .px(px(3.0))
                                .rounded_full()
                                .bg(cx.theme().primary)
                                .text_color(gpui_kit::white())
                                .text_size(px(9.0))
                                .font_weight(FontWeight::BOLD)
                                .shadow_md()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(badge_text),
                        )
                    }),
            )
            // Match the React panel width and maximum height.
            .when(self.is_open, |this| {
                this.child(
                    div()
                        .id("notification-center-panel")
                        .absolute()
                        .top(px(46.0))
                        .right_0()
                        .w(px(380.0))
                        .max_h(px(680.0))
                        .p_4()
                        .rounded(RADIUS_LG)
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .overflow_scroll()
                        // Panel Header
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                    .text_size(px(15.2))
                                    .font_weight(FontWeight::SEMIBOLD)
                                        .child("Task Center"),
                                )
                                .child(
                                    div().relative().when(clearable_count > 0, |menu| {
                                        menu.child(
                                            Button::new("btn-clear-menu")
                                                .secondary()
                                                .small()
                                                .label("Clear  v")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.clear_menu_open = !this.clear_menu_open;
                                                    cx.notify();
                                                })),
                                        )
                                        .when(self.clear_menu_open, |menu| {
                                            menu.child(
                                                div()
                                                    .absolute()
                                                    .top(px(34.0))
                                                    .right_0()
                                                    .w(px(230.0))
                                                    .p_1()
                                                    .rounded(RADIUS_MD)
                                                    .bg(cx.theme().background)
                                                    .border_1()
                                                    .border_color(cx.theme().border)
                                                    .shadow_lg()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(
                                                        Button::new("btn-clear-succeeded")
                                                            .ghost()
                                                            .small()
                                                            .label(format!("Clear succeeded ({succeeded_count})"))
                                                            .disabled(succeeded_count == 0)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.clear_tasks(true, cx);
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new("btn-clear-all")
                                                            .ghost()
                                                            .small()
                                                            .label(format!("Clear all completed ({clearable_count})"))
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.clear_tasks(false, cx);
                                                            })),
                                                    ),
                                            )
                                        })
                                    }),
                                ),
                        )
                        // Section 1: Needs Action
                        .when(!action_tasks.is_empty(), |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(COLOR_PROJECT_AMBER)
                                            .child("Needs action"),
                                    )
                                    .children(action_tasks.into_iter().enumerate().map(
                                        |(i, task)| {
                                            let task_id = task.id.clone();
                                            let dismiss_task_id = task_id.clone();
                                            let task_kind = task.kind.clone();
                                            let task_status = task.status.clone();
                                            let can_retry = task.file_path.is_some()
                                                && task.kind == sona_core::task_ledger::types::TaskLedgerKind::BatchImport;
                                            let retry_path = task.file_path.clone();
                                            let retry_project_id = task.tag_ids.first().cloned();
                                            div()
                                                .id(ElementId::from(format!("notif-action-{}", i)))
                                                .p_2p5()
                                                .rounded(RADIUS_SM)
                                                .bg(gpui_kit::transparent_black())
                                                .flex()
                                                .flex_col()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_start()
                                                        .justify_between()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                        .text_size(px(13.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(task.title.clone()),
                                                        )
                                                        .child(
                                                            Button::new(format!("dismiss-action-{i}"))
                                                                .ghost()
                                                                .xsmall()
                                                                .label("x")
                                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                                    this.dismiss(dismiss_task_id.clone(), cx);
                                                                })),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(cx.theme().muted)
                                                        .child(task_body(task)),
                                                )
                                                .when(task.stage.is_some() || task.error_message.is_some(), |card| {
                                                    card.child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .text_color(cx.theme().muted)
                                                            .child(task_detail(task)),
                                                    )
                                                })
                                                .when(can_retry, |card| {
                                                    let task_id = task_id.clone();
                                                    card.child(
                                                        Button::new(format!("retry-action-{i}"))
                                                            .primary()
                                                            .small()
                                                            .label("Retry")
                                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                                let _ = cx.global::<GlobalEventBus>().0.emit_typed(
                                                                    "notification:retry-batch",
                                                                    &serde_json::json!({ "taskId": task_id, "filePath": retry_path, "projectId": retry_project_id }),
                                                                );
                                                            })),
                                                    )
                                                })
                                                .when(task_kind == sona_core::task_ledger::types::TaskLedgerKind::Recovery
                                                    && task_status == TaskLedgerStatus::Recoverable, |card| {
                                                    card.child(
                                                        Button::new(format!("resume-action-{i}"))
                                                            .primary()
                                                            .small()
                                                            .label("Open Recovery")
                                                            .on_click(cx.listener(|_, _, _, cx| {
                                                                let _ = cx.global::<GlobalEventBus>().0.emit_typed(
                                                                    "notification:open-recovery",
                                                                    &serde_json::json!({}),
                                                                );
                                                            })),
                                                    )
                                                })
                                                .when(task_kind == sona_core::task_ledger::types::TaskLedgerKind::Automation, |card| {
                                                    card.child(
                                                        Button::new(format!("automation-action-{i}"))
                                                            .secondary()
                                                            .small()
                                                            .label("Open Automation")
                                                            .on_click(cx.listener(|_, _, _, cx| {
                                                                let _ = cx.global::<GlobalEventBus>().0.emit_typed(
                                                                    "notification:open-automation",
                                                                    &serde_json::json!({}),
                                                                );
                                                            })),
                                                    )
                                                })
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
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().primary)
                                            .child("Active"),
                                    )
                                    .children(active_tasks.into_iter().enumerate().map(
                                        |(i, task)| {
                                            let task_id = task.id.clone();
                                            let progress = progress_fraction(task.progress);
                                            div()
                                                .id(ElementId::from(format!("notif-active-{}", i)))
                                                .p_2p5()
                                                .rounded(RADIUS_SM)
                                                .bg(gpui_kit::transparent_black())
                                                .flex()
                                                .flex_col()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .child(
                                                            div()
                                                        .text_size(px(13.6))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                                .child(task.title.clone()),
                                                        )
                                                    .child(div().text_size(px(11.0)).child(format!("{:.0}%", progress * 100.0))),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(cx.theme().muted)
                                                        .child(task_body(task)),
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
                                                .when(task.cancelable && task.status != TaskLedgerStatus::CancelRequested, |card| {
                                                    let task_id = task_id.clone();
                                                    card.child(
                                                        Button::new(format!("cancel-active-{i}"))
                                                            .ghost()
                                                            .small()
                                                            .label("Cancel")
                                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                                let _ = cx.global::<GlobalEventBus>().0.emit_typed(
                                                                    "notification:cancel-task",
                                                                    &serde_json::json!({ "taskId": task_id }),
                                                                );
                                                            })),
                                                    )
                                                })
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
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().muted)
                                            .child("Recent"),
                                    )
                                    .children(info_tasks.into_iter().enumerate().map(
                                        |(i, task)| {
                                            let task_id = task.id.clone();
                                            div()
                                                .id(ElementId::from(format!("notif-info-{}", i)))
                                                .p_2p5()
                                                .rounded(RADIUS_SM)
                                                .bg(gpui_kit::transparent_black())
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(cx.theme().foreground)
                                                        .child(task.title.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(3.0))
                                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(task_body(task)))
                                                        .when(task.stage.is_some() || task.error_message.is_some(), |detail| {
                                                            detail.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(task_detail(task)))
                                                        }),
                                                )
                                                .when(task.status == TaskLedgerStatus::Succeeded, |row| {
                                                    row.child(
                                                        Icon::new(IconName::Check)
                                                            .size(px(12.0))
                                                            .text_color(cx.theme().success),
                                                    )
                                                })
                                                .when(task.status == TaskLedgerStatus::Cancelled, |row| {
                                                    row.child(
                                                        Icon::new(IconName::Close)
                                                            .size(px(12.0))
                                                            .text_color(cx.theme().muted),
                                                    )
                                                })
                                                .child(
                                                    Button::new(format!("dismiss-info-{i}"))
                                                        .ghost()
                                                        .xsmall()
                                                        .label("x")
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.dismiss(task_id.clone(), cx);
                                                        })),
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
    use super::{badge_label_for_count, progress_fraction};
    use core::prelude::v1::test;

    #[test]
    fn test_task_progress_clamping_logic() {
        assert_eq!(progress_fraction(0.0), 0.0);
        assert_eq!(progress_fraction(50.0), 0.5);
        assert_eq!(progress_fraction(100.0), 1.0);
        assert_eq!(progress_fraction(-25.0), 0.0);
        assert_eq!(progress_fraction(125.0), 1.0);
        assert_eq!(progress_fraction(f64::NAN), 0.0);
        assert_eq!(progress_fraction(f64::INFINITY), 0.0);
        assert_eq!(progress_fraction(f64::NEG_INFINITY), 0.0);
    }

    #[test]
    fn badge_label_matches_react_threshold() {
        assert_eq!(badge_label_for_count(0), "0");
        assert_eq!(badge_label_for_count(9), "9");
        assert_eq!(badge_label_for_count(10), "9+");
    }
}
