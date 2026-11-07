//! Recovery center for interrupted transcription and automation tasks.

use chrono::{Local, TimeZone};
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::recovery::types::{RecoveredQueueItem, RecoveryItemInput, RecoverySource};

use super::super::shell::MainShell;

pub struct RecoveryModal {
    pub is_open: bool,
    items: Vec<RecoveredQueueItem>,
    updated_at: Option<u64>,
    is_loading: bool,
    is_busy: bool,
    error: Option<String>,
}

impl RecoveryModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            items: Vec::new(),
            updated_at: None,
            is_loading: false,
            is_busy: false,
            error: None,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MainShell>) {
        self.is_open = true;
        self.load(cx);
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn load(&mut self, cx: &mut Context<MainShell>) {
        self.is_loading = true;
        self.error = None;
        cx.notify();

        let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = crate::platform::recovery_repository::load_snapshot(&provider).await;
            this.update(cx, |shell, cx| {
                let modal = &mut shell.recovery_modal;
                modal.is_loading = false;
                match result {
                    Ok(snapshot) => {
                        modal.items = snapshot.items;
                        modal.updated_at = snapshot.updated_at;
                        if modal.items.is_empty() {
                            modal.is_open = false;
                        }
                    }
                    Err(error) => modal.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn discard(&mut self, id: Option<String>, cx: &mut Context<MainShell>) {
        if self.is_busy {
            return;
        }
        let remaining = match id.as_deref() {
            Some(id) => self
                .items
                .iter()
                .filter(|item| item.id != id)
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        let removed_ids = self
            .items
            .iter()
            .filter(|item| id.as_deref().is_none_or(|id| item.id == id))
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        let inputs = match recovery_inputs(&remaining) {
            Ok(inputs) => inputs,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };

        self.is_busy = true;
        self.error = None;
        cx.notify();

        let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result =
                crate::platform::recovery_repository::save_snapshot(&provider, inputs).await;
            if result.is_ok() {
                for id in removed_ids {
                    let _ = services
                        .task_ledger
                        .remove_task(format!("recovery-{id}"))
                        .await;
                    let _ = services
                        .task_ledger
                        .remove_task(format!("batch-{id}"))
                        .await;
                }
            }
            this.update(cx, |shell, cx| {
                let modal = &mut shell.recovery_modal;
                modal.is_busy = false;
                match result {
                    Ok(snapshot) => {
                        modal.items = snapshot.items;
                        modal.updated_at = snapshot.updated_at;
                    }
                    Err(error) => modal.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn resume(&mut self, id: Option<String>, cx: &mut Context<MainShell>) {
        if self.is_busy {
            return;
        }
        let selected = self
            .items
            .iter()
            .filter(|item| {
                item.can_resume
                    && item.source == RecoverySource::BatchImport
                    && id.as_deref().is_none_or(|id| item.id == id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return;
        }
        let selected_ids = selected
            .iter()
            .map(|item| item.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let remaining = self
            .items
            .iter()
            .filter(|item| !selected_ids.contains(item.id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let inputs = match recovery_inputs(&remaining) {
            Ok(inputs) => inputs,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };

        self.is_busy = true;
        self.error = None;
        cx.notify();
        let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result =
                crate::platform::recovery_repository::save_snapshot(&provider, inputs).await;
            if result.is_ok() {
                for item in &selected {
                    let _ = services
                        .task_ledger
                        .remove_task(format!("recovery-{}", item.id))
                        .await;
                    let _ = services
                        .task_ledger
                        .remove_task(format!("batch-{}", item.id))
                        .await;
                }
            }
            this.update(cx, |shell, cx| {
                let modal = &mut shell.recovery_modal;
                modal.is_busy = false;
                match result {
                    Ok(snapshot) => {
                        modal.items = snapshot.items;
                        modal.updated_at = snapshot.updated_at;
                        let workspace = shell.create_workspace.clone();
                        workspace.update(cx, |workspace, cx| {
                            workspace
                                .set_tab(crate::views::create::CreateWorkspaceTab::BatchImport, cx);
                            workspace.batch_import.update(cx, |batch, cx| {
                                batch.enqueue_recovered_items(selected, cx);
                            });
                        });
                    }
                    Err(error) => modal.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let items = self.items.clone();
        let error = self.error.clone();
        let is_loading = self.is_loading;
        let is_busy = self.is_busy;
        let updated_at = self.updated_at;
        let batch = items
            .iter()
            .filter(|item| item.source == RecoverySource::BatchImport)
            .cloned()
            .collect::<Vec<_>>();
        let automation = items
            .iter()
            .filter(|item| item.source == RecoverySource::Automation)
            .cloned()
            .collect::<Vec<_>>();
        let other = items
            .iter()
            .filter(|item| {
                !matches!(
                    item.source,
                    RecoverySource::BatchImport | RecoverySource::Automation
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let has_missing_sources = items.iter().any(|item| !item.can_resume);
        let has_unsupported_source = items
            .iter()
            .any(|item| item.source != RecoverySource::BatchImport);
        let has_error = error.is_some();

        div().when(self.is_open, |this| {
            this.child(
                div()
                    .id("recovery-backdrop")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.4))
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.recovery_modal.close();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("recovery-panel")
                            .flex()
                            .flex_col()
                            .w(px(1104.0))
                            .max_w(px(1200.0))
                            .h(px(704.0))
                            .max_h(px(780.0))
                            .overflow_hidden()
                            .rounded_xl()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_xl()
                            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .justify_between()
                                    .gap_4()
                                    .p_6()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .text_color(cx.theme().primary)
                                                    .child(Icon::new(LucideIcon::RefreshCw).size(px(16.0)))
                                                    .child("RECOVERY"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(24.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Interrupted Batch Recovery"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Review interrupted imports and automation tasks."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_end()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("recovery-resume-all")
                                                            .outline()
                                                            .small()
                                                            .label("Resume All")
                                                            .disabled(is_busy || items.is_empty() || has_missing_sources || has_unsupported_source)
                                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                                shell.recovery_modal.resume(None, cx);
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new("recovery-discard-all")
                                                            .outline()
                                                            .small()
                                                            .label("Discard All")
                                                            .disabled(is_busy || items.is_empty())
                                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                                shell.recovery_modal.discard(None, cx);
                                                            })),
                                                    ),
                                            )
                                            .child(
                                                Button::new("recovery-close")
                                                    .ghost()
                                                    .small()
                                                    .label("Close")
                                                    .on_click(cx.listener(|shell, _, _, cx| {
                                                        shell.recovery_modal.close();
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .px_8()
                                    .pt_3()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child("LAST RECOVERED")
                                    .child(updated_at.map(format_timestamp).unwrap_or_else(|| "Unknown".into())),
                            )
                            .when_some(error, |this, error| {
                                this.child(
                                    div()
                                        .mx_8()
                                        .mt_3()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().danger.opacity(0.1))
                                        .text_color(cx.theme().danger)
                                        .child(error),
                                )
                            })
                            .child(
                                div()
                                    .id("recovery-content")
                                    .flex()
                                    .flex_col()
                                    .gap_6()
                                    .p_8()
                                    .overflow_y_scroll()
                                    .when(is_loading && items.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .py_8()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(Icon::new(LucideIcon::LoaderCircle).size(px(16.0)))
                                                .child("Loading recovery state..."),
                                        )
                                    })
                                    .when(!is_loading && items.is_empty() && !has_error, |this| {
                                        this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .gap_2()
                                                .py_8()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(Icon::new(LucideIcon::CircleCheck).size(px(24.0)))
                                                .child("No interrupted tasks"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .gap_4()
                                            .child(overview_card("Batch Import", "Interrupted files from manual imports.", &batch, LucideIcon::FileStack, cx))
                                            .child(overview_card("Automation", "Files interrupted while processing an automation rule.", &automation, LucideIcon::Workflow, cx)),
                                    )
                                    .when(!batch.is_empty(), |this| this.child(section("Batch Import", "Resume interrupted file imports.", batch, is_busy, cx)))
                                    .when(!automation.is_empty(), |this| this.child(section("Automation", "Resume files processed by automation rules.", automation, is_busy, cx)))
                                    .when(!other.is_empty(), |this| this.child(section("Other Recovery", "Additional recoverable work.", other, is_busy, cx))),
                            ),
                    ),
            )
        })
    }
}

fn recovery_inputs(items: &[RecoveredQueueItem]) -> Result<Vec<RecoveryItemInput>, String> {
    serde_json::from_value(serde_json::to_value(items).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn format_timestamp(timestamp: u64) -> String {
    Local
        .timestamp_millis_opt(timestamp as i64)
        .single()
        .map(|time| time.format("%x %X").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn overview_card(
    title: &'static str,
    description: &'static str,
    items: &[RecoveredQueueItem],
    icon: LucideIcon,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    let draft_count = items
        .iter()
        .filter(|item| item.history_id.is_some() || !item.segments.is_empty())
        .count();
    div()
        .flex()
        .flex_col()
        .gap_3()
        .min_h(px(180.0))
        .flex_1()
        .p_5()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary.opacity(0.16))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(metric(icon, format!("{} pending", items.len()), cx))
                .child(metric(
                    LucideIcon::Clock3,
                    format!("{draft_count} drafts"),
                    cx,
                )),
        )
        .child(
            div()
                .text_size(px(17.0))
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}

fn metric(icon: LucideIcon, label: String, cx: &Context<MainShell>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_full()
        .bg(cx.theme().primary.opacity(0.08))
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .child(Icon::new(icon).size(px(14.0)))
        .child(label)
}

fn section(
    title: &'static str,
    description: &'static str,
    items: Vec<RecoveredQueueItem>,
    is_busy: bool,
    cx: &mut Context<MainShell>,
) -> impl IntoElement {
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
                        .text_size(px(17.0))
                        .font_weight(FontWeight::BOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(description),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .rounded_lg()
                .overflow_hidden()
                .border_1()
                .border_color(cx.theme().border)
                .children(items.into_iter().map(|item| {
                    let item_id = item.id.clone();
                    let can_resume = item.can_resume && item.source == RecoverySource::BatchImport;
                    let resume_id = item.id.clone();
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_4()
                        .p_4()
                        .bg(cx.theme().background)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .child(item.filename.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_2()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "{} · {} · {}",
                                            source_label(item.source),
                                            stage_label(item.last_known_stage),
                                            format_timestamp(item.updated_at)
                                        ))
                                        .when(
                                            item.history_id.is_some() || !item.segments.is_empty(),
                                            |this| this.child("Partial draft"),
                                        )
                                        .when(!item.can_resume, |this| {
                                            this.child(
                                                div()
                                                    .text_color(cx.theme().warning)
                                                    .child("Source file missing"),
                                            )
                                        }),
                                )
                                .child(div().text_color(cx.theme().muted_foreground).child(
                                    if !item.can_resume {
                                        "The original source file is unavailable."
                                    } else if item.source == RecoverySource::Automation {
                                        "Interrupted while running an automation rule."
                                    } else {
                                        "Interrupted during batch transcription."
                                    },
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new(format!("recovery-resume-{item_id}"))
                                        .outline()
                                        .small()
                                        .label("Resume")
                                        .disabled(is_busy || !can_resume)
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell
                                                .recovery_modal
                                                .resume(Some(resume_id.clone()), cx);
                                        })),
                                )
                                .child(
                                    Button::new(format!("recovery-discard-{item_id}"))
                                        .outline()
                                        .small()
                                        .label("Discard")
                                        .disabled(is_busy)
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell.recovery_modal.discard(Some(item_id.clone()), cx);
                                        })),
                                ),
                        )
                })),
        )
}

fn source_label(source: RecoverySource) -> &'static str {
    match source {
        RecoverySource::BatchImport => "Batch Import",
        RecoverySource::Automation => "Automation",
        RecoverySource::TranscriptEdit => "Transcript Edit",
    }
}

fn stage_label(stage: sona_core::recovery::types::RecoveryItemStage) -> &'static str {
    match stage {
        sona_core::recovery::types::RecoveryItemStage::Queued => "Queued",
        sona_core::recovery::types::RecoveryItemStage::Transcribing => "Transcribing",
        sona_core::recovery::types::RecoveryItemStage::Polishing => "Polishing",
        sona_core::recovery::types::RecoveryItemStage::Translating => "Translating",
        sona_core::recovery::types::RecoveryItemStage::Exporting => "Exporting",
    }
}

#[cfg(test)]
mod tests {
    use super::recovery_inputs;
    use serde_json::json;
    use sona_core::recovery::types::RecoveredQueueItem;

    #[test]
    fn recovery_save_conversion_preserves_draft_and_automation_metadata() {
        let item: RecoveredQueueItem = serde_json::from_value(json!({
            "id": "recovery-1",
            "filename": "meeting.wav",
            "filePath": "C:/audio/meeting.wav",
            "source": "automation",
            "resolution": "pending",
            "progress": 35.0,
            "segments": [{
                "id": "segment-1",
                "text": "partial transcript",
                "start": 0.0,
                "end": 1.2,
                "isFinal": true
            }],
            "tagIds": ["project-1"],
            "historyId": "history-1",
            "historyTitle": "Meeting",
            "lastKnownStage": "translating",
            "updatedAt": 1700000000000_u64,
            "hasSourceFile": true,
            "canResume": true,
            "automationRuleId": "rule-1",
            "automationRuleName": "Inbox",
            "resolvedConfigSnapshot": {"language": "en"},
            "automationResolutionSnapshot": {"projectId": "project-1"},
            "exportConfig": {"format": "srt"},
            "stageConfig": {"translate": true},
            "sourceFingerprint": "fingerprint-1",
            "fileStat": {"size": 1024, "mtimeMs": 1700000000000_u64},
            "exportFileNamePrefix": "meeting",
            "attemptCount": 2,
            "lastError": "interrupted",
            "retryable": true
        }))
        .unwrap();

        let input = recovery_inputs(&[item]).unwrap();
        let value = serde_json::to_value(input).unwrap();
        let saved = &value[0];
        assert_eq!(saved["segments"][0]["text"], "partial transcript");
        assert_eq!(saved["historyId"], "history-1");
        assert_eq!(saved["automationRuleId"], "rule-1");
        assert_eq!(saved["resolvedConfigSnapshot"]["language"], "en");
        assert_eq!(saved["exportConfig"]["format"], "srt");
        assert_eq!(saved["fileStat"]["size"], 1024);
    }
}
