use std::path::PathBuf;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::storage_usage::{SQLiteUsageSummary, StorageUsageSnapshot};

use super::SettingsModal;

#[derive(Default)]
pub struct StorageSettingsTab {
    pub directories: Option<crate::platform::storage::location::StorageDirectoriesInfo>,
    pub usage: Option<StorageUsageSnapshot>,
    pub loading: bool,
    pub migration_busy: bool,
    pub migration_progress: Option<crate::platform::storage::location::StorageMigrationProgress>,
    pub error: Option<String>,
    pub retention_days: Option<u64>,
    pub active_history_id: Option<String>,
    pub cleanup_busy: bool,
    pub cleanup_result: Option<String>,
    pub pending_migration: Option<(bool, String)>,
    pub cleanup_pending: Option<sona_core::history::HistoryAudioCleanupRequest>,
    pub pending_reset: Option<bool>,
}

impl StorageSettingsTab {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_storage_state(&mut self, cx: &mut Context<SettingsModal>) {
        self.loading = true;
        self.error = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                let directories = crate::platform::storage::location::get_storage_directories_info(
                    &crate::platform::storage::location::default_app_local_data_dir(),
                )?;
                let context = services.sqlite.current_context()?;
                let usage = crate::platform::storage::usage::get_usage_snapshot(
                    &context,
                    Some(PathBuf::from(&directories.models_dir)),
                )
                .await?;
                Ok::<_, String>((directories, usage))
            }
            .await;
            let _ = this.update(cx, |modal, cx| {
                modal.storage.loading = false;
                match result {
                    Ok((directories, usage)) => {
                        modal.storage.directories = Some(directories);
                        modal.storage.usage = Some(usage);
                        modal.storage.error = None;
                    }
                    Err(error) => modal.storage.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_storage_retention(&mut self, days: Option<u64>, cx: &mut Context<SettingsModal>) {
        self.retention_days = days;
        let value = serde_json::to_value(days).unwrap_or(serde_json::Value::Null);
        modal_persist_preference("historyAudioRetentionDays", value, cx);
        cx.notify();
    }

    pub fn preview_storage_cleanup(
        &mut self,
        days: u64,
        active_history_id: Option<String>,
        cx: &mut Context<SettingsModal>,
    ) {
        self.cleanup_busy = true;
        self.cleanup_result = None;
        let history = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .history
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let request = sona_core::history::HistoryAudioCleanupRequest {
                retention_days: Some(days),
                exclude_history_id: active_history_id,
            };
            let result = history.preview_audio_cleanup(request.clone()).await;
            this.update(cx, |modal, cx| {
                modal.storage.cleanup_busy = false;
                match result {
                    Ok(report)
                        if report.eligible_count > 0
                            || report.removed_count > 0
                            || report.missing_marked_count > 0 =>
                    {
                        modal.storage.cleanup_pending = Some(request);
                        modal.storage.cleanup_result = Some(storage_cleanup_summary(&report));
                    }
                    Ok(report) => {
                        modal.storage.cleanup_result = Some(storage_cleanup_summary(&report))
                    }
                    Err(error) => modal.storage.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn confirm_storage_cleanup(&mut self, cx: &mut Context<SettingsModal>) {
        let Some(request) = self.cleanup_pending.take() else {
            return;
        };
        self.cleanup_busy = true;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = services.history.cleanup_audio(request).await;
            this.update(cx, |modal, cx| {
                modal.storage.cleanup_busy = false;
                match result {
                    Ok(report) => {
                        modal.storage.cleanup_result = Some(storage_cleanup_summary(&report));
                        modal.storage.load_storage_state(cx);
                    }
                    Err(error) => modal.storage.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn choose_storage_directory(
        &mut self,
        data_directory: bool,
        cx: &mut Context<SettingsModal>,
    ) {
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let selected = rfd::AsyncFileDialog::new()
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf());
            let Some(path) = selected else { return };
            if let Err(error) =
                crate::platform::storage::location::check_storage_can_migrate(&services).await
            {
                this.update(cx, |modal, cx| {
                    modal.storage.error = Some(error);
                    cx.notify();
                })
                .ok();
                return;
            }
            this.update(cx, |modal, cx| {
                let path = path.to_string_lossy().into_owned();
                modal.storage.pending_migration = Some((data_directory, path));
                modal.storage.confirm_storage_migration(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn confirm_storage_migration(&mut self, cx: &mut Context<SettingsModal>) {
        let Some((data_directory, path)) = self.pending_migration.take() else {
            return;
        };
        self.migration_busy = true;
        self.migration_progress = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let (_progress_tx, mut progress_rx) = tokio::sync::watch::channel(
                crate::platform::storage::location::StorageMigrationProgress {
                    phase: "Starting migration...".to_string(),
                    percent: 0.0,
                    current_file: String::new(),
                    copied_bytes: 0,
                    total_bytes: 0,
                },
            );
            let progress_task = {
                let this = this.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    while progress_rx.changed().await.is_ok() {
                        let progress = progress_rx.borrow_and_update().clone();
                        let _ = this.update(cx, |modal, cx| {
                            modal.storage.migration_progress = Some(progress);
                            cx.notify();
                        });
                    }
                })
            };
            let result = if data_directory {
                crate::platform::storage::location::migrate_data_directory(
                    &services,
                    PathBuf::from(&path),
                    true,
                )
                .await
            } else {
                crate::platform::storage::location::set_models_directory(
                    &services,
                    PathBuf::from(&path),
                    true,
                )
                .await
            };
            drop(progress_task);
            this.update(cx, |modal, cx| {
                modal.storage.migration_busy = false;
                match result {
                    Ok(directories) => {
                        modal.storage.directories = Some(directories);
                        modal.storage.pending_migration = None;
                        modal.storage.load_storage_state(cx);
                    }
                    Err(error) => {
                        modal.storage.error = Some(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn reset_storage_directory(
        &mut self,
        data_directory: bool,
        cx: &mut Context<SettingsModal>,
    ) {
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let Some(directories) = self.directories.as_ref() else {
            return;
        };
        let _target = if data_directory {
            directories.default_data_dir.clone()
        } else {
            directories.default_models_dir.clone()
        };
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            if let Err(error) =
                crate::platform::storage::location::check_storage_can_migrate(&services).await
            {
                this.update(cx, |modal, cx| {
                    modal.storage.error = Some(error);
                    cx.notify();
                })
                .ok();
                return;
            }
            this.update(cx, |modal, cx| {
                modal.storage.pending_reset = Some(data_directory);
                modal.storage.confirm_storage_directory_reset(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn confirm_storage_directory_reset(&mut self, cx: &mut Context<SettingsModal>) {
        let Some(data_directory) = self.pending_reset.take() else {
            return;
        };
        self.migration_busy = true;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = if data_directory {
                crate::platform::storage::location::reset_data_directory(&services).await
            } else {
                crate::platform::storage::location::reset_models_directory(&services).await
            };
            this.update(cx, |modal, cx| {
                modal.storage.migration_busy = false;
                match result {
                    Ok(directories) => {
                        modal.storage.directories = Some(directories);
                        modal.storage.load_storage_state(cx);
                    }
                    Err(error) => modal.storage.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn render(&mut self, cx: &mut Context<SettingsModal>) -> impl IntoElement {
        let directories = self.directories.as_ref();
        let usage = self.usage.as_ref();
        let loading = self.loading;
        let migration_busy = self.migration_busy;
        let migration_progress = self.migration_progress.as_ref();
        let error = self.error.as_deref();
        let retention_days = self.retention_days;
        let active_history_id = self.active_history_id.as_deref();
        let cleanup_busy = self.cleanup_busy;
        let cleanup_result = self.cleanup_result.as_deref();

        div()
            .id("settings-storage-panel")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(32.0))
            .py(px(24.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(900.0))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .pb_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Data & Storage"),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .text_size(px(13.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Review local data usage and manage storage locations."),
                            ),
                    )
                    .when_some(error, |panel, error| {
                        panel.child(
                            div()
                                .p_3()
                                .rounded_md()
                                .bg(cx.theme().danger.opacity(0.08))
                                .text_color(cx.theme().danger)
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(format!("Storage information unavailable: {error}"))
                                .child(
                                    Button::new("settings-storage-retry")
                                        .outline()
                                        .small()
                                        .label("Retry")
                                        .on_click(cx.listener(|modal, _, _, cx| {
                                            modal.storage.load_storage_state(cx);
                                        })),
                                ),
                        )
                    })
                    .child(
                        section("Storage Locations", "Application data and speech recognition models.", LucideIcon::Folder, cx)
                            .child(location_row("Data Directory", directories.map(|item| item.data_dir.as_str()), directories.is_some_and(|item| item.is_custom_data_dir), "Stores databases, history recordings, speaker profiles, and recovery snapshots.", true, migration_busy, "settings-storage-open-data", cx))
                            .child(location_row("Models Directory", directories.map(|item| item.models_dir.as_str()), directories.is_some_and(|item| item.is_custom_models_dir), "Stores offline speech recognition, VAD, and punctuation models.", false, migration_busy, "settings-storage-open-models", cx))
                            .when_some(migration_progress, |this, progress| this.child(div().flex().flex_col().gap_1().pt_2().child(div().flex().justify_between().text_size(px(12.0)).child(format!("{} {}", progress.phase, progress.current_file)).child(format!("{:.0}%", progress.percent))).child(div().h(px(4.0)).w_full().rounded_full().bg(cx.theme().muted).child(div().h_full().w(px(6.0 + 820.0 * progress.percent.clamp(0.0, 100.0) as f32 / 100.0)).rounded_full().bg(cx.theme().primary))))),
                    )
                    .child(
                        section("Data Usage", "Local audio, database files, models, temporary files, and other app data.", LucideIcon::HardDrive, cx)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .p_3()
                                    .rounded_md()
                                    .bg(cx.theme().group_box)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Total local usage"))
                                            .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child(usage.map(|value| format_bytes(value.total_bytes)).unwrap_or_else(|| if loading { "Loading…".into() } else { "Unavailable".into() })))
                                            .when_some(usage, |this, value| this.child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(format!("Updated {}", value.generated_at)))),
                                    )
                                    .child(
                                        Button::new("settings-storage-refresh")
                                            .outline()
                                            .small()
                                            .icon(Icon::new(LucideIcon::RefreshCw))
                                            .label(if loading { "Refreshing…" } else { "Refresh" })
                                            .disabled(loading)
                                            .on_click(cx.listener(|modal, _, _, cx| modal.storage.load_storage_state(cx))),
                                    ),
                            )
                            .when_some(usage, |this, usage| {
                                this.child(usage_metrics(usage, cx))
                                    .child(sqlite_breakdown(&usage.categories.database.sqlite, cx))
                            })
                            .when(usage.is_none() && loading, |this| this.child(div().p_3().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Loading storage measurements…")))
                            .when(usage.is_none() && !loading && error.is_none(), |this| this.child(div().p_3().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("No storage measurements are available."))),
                    )
                    .child(history_audio_section(
                        retention_days,
                        active_history_id,
                        cleanup_busy,
                        cleanup_result,
                        cx,
                    )),
            )
            .into_any_element()
    }
}

pub fn storage_cleanup_summary(report: &sona_core::history::HistoryAudioCleanupReport) -> String {
    let mut parts = Vec::new();
    if report.removed_count > 0 {
        parts.push(format!("{} files removed", report.removed_count));
    }
    if report.removed_bytes > 0 {
        parts.push(format!("{} freed", format_bytes(report.removed_bytes)));
    }
    if report.missing_marked_count > 0 {
        parts.push(format!("{} missing", report.missing_marked_count));
    }
    if report.failed_count > 0 {
        parts.push(format!("{} failed", report.failed_count));
    }
    if report.skipped_active_count > 0 {
        parts.push(format!("{} active skipped", report.skipped_active_count));
    }
    if parts.is_empty() {
        "No audio files need cleanup.".to_string()
    } else {
        parts.join(" · ")
    }
}

fn modal_persist_preference(key: &str, value: serde_json::Value, cx: &mut Context<SettingsModal>) {
    let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
    let key_owned = key.to_string();
    cx.spawn(async move |_this, _cx: &mut AsyncApp| {
        let _ = services.config.set_setting(&key_owned, &value);
    })
    .detach();
}

fn history_audio_section(
    retention_days: Option<u64>,
    active_history_id: Option<&str>,
    cleanup_busy: bool,
    cleanup_result: Option<&str>,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    let active_history_id = active_history_id.map(ToOwned::to_owned);
    let options: [(Option<u64>, &'static str); 7] = [
        (None, "Keep forever"),
        (Some(0), "Remove immediately"),
        (Some(7), "7 days"),
        (Some(30), "30 days"),
        (Some(90), "90 days"),
        (Some(180), "180 days"),
        (Some(365), "365 days"),
    ];
    section(
        "History Audio",
        "Cleanup affects managed audio files only. Saved transcripts, summaries, and history rows remain available.",
        LucideIcon::Music,
        cx,
    )
    .child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .pt_3()
            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Audio retention"))
            .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child("Automatic cleanup runs after startup and at most once per day while Sona is open."))
            .child(div().flex().flex_wrap().gap_1().children(options.into_iter().map(|(days, label)| {
                let selected = days == retention_days;
                Button::new(format!("settings-storage-retention-{}", days.map_or_else(|| "forever".to_string(), |value| value.to_string())))
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.outline())
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |modal, _, _, cx| modal.storage.set_storage_retention(days, cx)))
            })))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_3()
                    .child(div().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Manual cleanup"))
                        .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(if retention_days.is_none() { "Choose a finite retention period before cleaning audio." } else { "Preview eligible files before removal; the active history item is excluded." })))
                    .child(
                        Button::new("settings-storage-clean-now")
                            .outline()
                            .small()
                            .icon(Icon::new(LucideIcon::Trash))
                            .label(if cleanup_busy { "Cleaning…" } else { "Clean Now" })
                            .disabled(cleanup_busy || retention_days.is_none())
                            .on_click(cx.listener(move |modal, _, _, cx| {
                                if let Some(days) = retention_days {
                                    modal.storage.preview_storage_cleanup(days, active_history_id.clone(), cx);
                                }
                            })),
                    ),
            )
            .when_some(cleanup_result, |this, result| {
                this.child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child(result.to_string()))
            }),
    )
}

fn section(
    title: &'static str,
    description: &'static str,
    icon: LucideIcon,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(Icon::new(icon).size(px(18.0)))
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}

fn location_row(
    title: &'static str,
    path: Option<&str>,
    custom: bool,
    hint: &'static str,
    data_directory: bool,
    busy: bool,
    id: &'static str,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    let path_available = path.is_some();
    let path = path.unwrap_or("Loading…").to_string();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .child(title),
                        )
                        .when(custom, |row| {
                            row.child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(cx.theme().primary)
                                    .child("Custom"),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(path.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new(format!("{id}-change"))
                        .outline()
                        .small()
                        .label("Change…")
                        .disabled(busy)
                        .on_click(cx.listener(move |modal, _, _, cx| {
                            modal.storage.choose_storage_directory(data_directory, cx)
                        })),
                )
                .when(custom, |row| {
                    row.child(
                        Button::new(format!("{id}-restore"))
                            .ghost()
                            .small()
                            .label("Restore Default")
                            .disabled(busy)
                            .on_click(cx.listener(move |modal, _, _, cx| {
                                modal.storage.reset_storage_directory(data_directory, cx)
                            })),
                    )
                })
                .child(
                    Button::new(id)
                        .outline()
                        .small()
                        .icon(Icon::new(LucideIcon::FolderOpen))
                        .label("Open Folder")
                        .disabled(!path_available)
                        .on_click(cx.listener(move |_, _, _, _| {
                            if let Err(error) =
                                crate::platform::storage::location::open_storage_path(path.clone())
                            {
                                log::error!("Failed to open storage path: {error}");
                            }
                        })),
                ),
        )
}

fn usage_metrics(usage: &StorageUsageSnapshot, cx: &Context<SettingsModal>) -> gpui_kit::Div {
    let categories = &usage.categories;
    let sqlite = &categories.database.sqlite;
    let metrics = [
        (
            LucideIcon::Music,
            "Audio",
            categories.audio.bytes,
            format!(
                "History {} · Speaker samples {}",
                format_bytes(categories.audio.history_audio_bytes),
                format_bytes(categories.audio.speaker_sample_bytes)
            ),
        ),
        (
            LucideIcon::Database,
            "Database",
            categories.database.bytes,
            format!(
                "SQLite data {} · indexes {}",
                format_bytes(sqlite.data_bytes),
                format_bytes(sqlite.index_bytes)
            ),
        ),
        (
            LucideIcon::Box,
            "Models",
            categories.models.bytes,
            format!("{} files", categories.models.file_count),
        ),
        (
            LucideIcon::Clock,
            "Temporary",
            categories.temporary.bytes,
            format!("{} files", categories.temporary.file_count),
        ),
    ];
    div()
        .grid()
        .grid_cols(4)
        .gap_2()
        .pt_2()
        .children(metrics.into_iter().map(|(icon, label, bytes, detail)| {
            div()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(icon)
                                .size(px(14.0))
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(label),
                        ),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format_bytes(bytes)),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                )
        }))
}

fn sqlite_breakdown(sqlite: &SQLiteUsageSummary, cx: &Context<SettingsModal>) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .pt_2()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("SQLite Breakdown"),
        )
        .children(
            [
                ("Table data", format_bytes(sqlite.data_bytes)),
                ("SQLite indexes", format_bytes(sqlite.index_bytes)),
                ("Free pages", format_bytes(sqlite.free_page_bytes)),
                (
                    "Indexed structures",
                    format!("{} structures", sqlite.index_entries.len()),
                ),
                (
                    "Main DB + WAL/SHM",
                    format_bytes(
                        sqlite.main_db_bytes + sqlite.main_wal_bytes + sqlite.main_shm_bytes,
                    ),
                ),
            ]
            .into_iter()
            .map(|(label, value)| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(12.0))
                    .child(div().text_color(cx.theme().muted_foreground).child(label))
                    .child(div().font_weight(FontWeight::MEDIUM).child(value))
            }),
        )
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}
