//! Projects main header with title, workflow quick actions, and summary statistics.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::history::{HistoryWorkspaceScope, HistoryWorkspaceSummary};
use sona_core::project::ProjectRecord;

use crate::theme::tokens::*;
use crate::views::create::workspace::CreateWorkspaceTab;
use crate::views::shell::{AppMode, MainShell};

pub struct ProjectsHeader;

impl ProjectsHeader {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        scope: &HistoryWorkspaceScope,
        project: Option<&ProjectRecord>,
        summary: Option<&HistoryWorkspaceSummary>,
        trash_count: usize,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let is_all_scope = matches!(scope, HistoryWorkspaceScope::All);
        let is_inbox_scope = matches!(scope, HistoryWorkspaceScope::Inbox);
        let is_trash_scope = matches!(scope, HistoryWorkspaceScope::Trash);
        let show_workflow_actions = !is_all_scope && !is_trash_scope;

        let (header_title, header_description) = if is_all_scope {
            (
                "All Items".to_string(),
                "Browse everything across Inbox and your projects.".to_string(),
            )
        } else if is_inbox_scope {
            (
                "Inbox".to_string(),
                "Inbox collects recordings and imports without a project.".to_string(),
            )
        } else if is_trash_scope {
            (
                "Trash".to_string(),
                "Restore items or delete them permanently.".to_string(),
            )
        } else if let Some(p) = project {
            (p.name.clone(), p.description.clone())
        } else {
            (
                "Inbox".to_string(),
                "Inbox collects recordings and imports without a project.".to_string(),
            )
        };

        let scope_icon = if is_all_scope {
            Icon::new(LucideIcon::ListFilter)
                .size(px(20.0))
                .text_color(cx.theme().muted)
                .into_any_element()
        } else if is_inbox_scope {
            Icon::new(LucideIcon::Inbox)
                .size(px(20.0))
                .text_color(cx.theme().muted)
                .into_any_element()
        } else if is_trash_scope {
            Icon::new(LucideIcon::Trash)
                .size(px(20.0))
                .text_color(cx.theme().danger)
                .into_any_element()
        } else {
            let color = project
                .and_then(|p| p.color.as_deref())
                .unwrap_or("#64748b");
            let color = color_from_hex_str(color);
            let project_icon = project.and_then(|p| p.icon.as_deref());
            let system_icon = project_icon
                .and_then(|icon| icon.strip_prefix("system:"))
                .and_then(|icon| match icon {
                    "mic" => Some(LucideIcon::Mic),
                    "file" => Some(LucideIcon::FileText),
                    "folder" => Some(LucideIcon::Folder),
                    "code" => Some(LucideIcon::Code),
                    _ => None,
                })
                .unwrap_or(LucideIcon::Folder);
            div()
                .size(px(34.0))
                .rounded(px(8.0))
                .bg(color.opacity(0.14))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    if let Some(emoji) =
                        project_icon.filter(|icon| !icon.is_empty() && !icon.starts_with("system:"))
                    {
                        div()
                            .text_size(px(21.0))
                            .child(emoji.to_string())
                            .into_any_element()
                    } else {
                        Icon::new(system_icon)
                            .size(px(22.0))
                            .text_color(color)
                            .into_any_element()
                    },
                )
                .into_any_element()
        };

        let (total_items_str, total_duration_str, latest_activity_str, type_split_str) =
            if let Some(s) = summary {
                let dur = format_summary_duration(s.total_duration);
                let latest = if let Some(ts) = s.latest_timestamp {
                    format_relative_timestamp(ts)
                } else {
                    "No activity yet".to_string()
                };
                let split = format!(
                    "{} recordings / {} imports",
                    s.recording_count, s.batch_count
                );
                (s.total_items.to_string(), dur, latest, split)
            } else {
                (
                    "0".to_string(),
                    "0m".to_string(),
                    "No activity yet".to_string(),
                    "0 recordings / 0 imports".to_string(),
                )
            };

        div()
            .flex()
            .flex_col()
            .gap(SPACING_SM)
            .px(px(24.0))
            .py(px(16.0))
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            // Top section: heading & entry action buttons
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(SPACING_MD)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .min_w_0()
                            .child(
                                div().flex().items_center().gap_2().child(scope_icon).child(
                                    div()
                                        .text_size(px(18.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().foreground)
                                        .child(header_title),
                                ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(cx.theme().muted)
                                    .child(header_description),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(show_workflow_actions, |actions| {
                                actions
                                    .child(
                                        Button::new("btn-header-live-record")
                                            .ghost()
                                            .small()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .icon(LucideIcon::Mic)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .tooltip("Start Live Record")
                                            .accessibility_label("Start Live Record")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mode = AppMode::Create;
                                                this.create_workspace.update(cx, |ws, cx| {
                                                    ws.active_tab = CreateWorkspaceTab::LiveRecord;
                                                    cx.notify();
                                                });
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("btn-header-batch-import")
                                            .ghost()
                                            .small()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .icon(LucideIcon::FileText)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .tooltip("Open Batch Import")
                                            .accessibility_label("Open Batch Import")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mode = AppMode::Create;
                                                this.create_workspace.update(cx, |ws, cx| {
                                                    ws.active_tab = CreateWorkspaceTab::BatchImport;
                                                    cx.notify();
                                                });
                                                cx.notify();
                                            })),
                                    )
                            })
                            .when_some(project, |actions, p| {
                                let p_clone = p.clone();
                                actions.child(
                                    Button::new("btn-header-project-settings")
                                        .ghost()
                                        .small()
                                        .w(px(36.0))
                                        .h(px(36.0))
                                        .icon(LucideIcon::Settings)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .tooltip("Project Settings")
                                        .accessibility_label("Project Settings")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.project_dialogs.update(cx, |dialogs, cx| {
                                                dialogs.open_settings(&p_clone, window, cx);
                                            });
                                        })),
                                )
                            })
                            .when(is_trash_scope, |actions| {
                                actions.child(
                                    Button::new("btn-header-empty-trash")
                                        .danger()
                                        .small()
                                        .w(px(36.0))
                                        .h(px(36.0))
                                        .icon(LucideIcon::Trash)
                                        .tooltip("Empty Trash")
                                        .accessibility_label("Empty Trash")
                                        .disabled(trash_count == 0)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.selection_bar.request_empty_trash(trash_count, cx);
                                        })),
                                )
                            }),
                    ),
            )
            // Bottom section: Summary statistic chips
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .mt(px(2.0))
                    .child(stat_pill("Items", &total_items_str, cx))
                    .child(stat_pill("Total duration", &total_duration_str, cx))
                    .child(stat_pill("Latest activity", &latest_activity_str, cx))
                    .child(stat_pill("Type split", &type_split_str, cx)),
            )
    }
}

impl Default for ProjectsHeader {
    fn default() -> Self {
        Self::new()
    }
}

fn stat_pill(label: &str, value: &str, cx: &Context<MainShell>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px(px(10.0))
        .py(px(4.0))
        .rounded_full()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().muted)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(value.to_string()),
        )
}

fn format_summary_duration(seconds: f64) -> String {
    let total_minutes = (seconds / 60.0).round() as u64;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{total_minutes}m")
    }
}

fn format_relative_timestamp(timestamp_ms: u64) -> String {
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let diff_secs = if now > timestamp_ms {
        (now - timestamp_ms) / 1000
    } else {
        0
    };
    if diff_secs < 60 {
        "Just now".to_string()
    } else if diff_secs < 3600 {
        format!("{}m ago", diff_secs / 60)
    } else if diff_secs < 86400 {
        format!("{}h ago", diff_secs / 3600)
    } else if diff_secs < 604800 {
        format!("{}d ago", diff_secs / 86400)
    } else {
        if let Some(dt) = chrono::DateTime::from_timestamp_millis(timestamp_ms as i64) {
            dt.format("%b %d, %Y").to_string()
        } else {
            "Recently".to_string()
        }
    }
}

fn color_from_hex_str(hex: &str) -> Hsla {
    hex_to_hsla(hex).unwrap_or_else(|| hsla(215.0 / 360.0, 0.16, 0.47, 1.0))
}
