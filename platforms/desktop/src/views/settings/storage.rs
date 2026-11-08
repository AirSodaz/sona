use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::storage_usage::{SQLiteUsageSummary, StorageUsageSnapshot};

pub(super) fn storage_panel(
    directories: Option<&crate::platform::storage::location::StorageDirectoriesInfo>,
    usage: Option<&StorageUsageSnapshot>,
    loading: bool,
    migration_busy: bool,
    migration_progress: Option<&crate::platform::storage::location::StorageMigrationProgress>,
    error: Option<&str>,
    retention_days: Option<u64>,
    active_history_id: Option<&str>,
    cleanup_busy: bool,
    cleanup_result: Option<&str>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
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
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.settings_modal.load_storage_state(cx);
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
                                        .on_click(cx.listener(|shell, _, _, cx| shell.settings_modal.load_storage_state(cx))),
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

fn history_audio_section(
    retention_days: Option<u64>,
    active_history_id: Option<&str>,
    cleanup_busy: bool,
    cleanup_result: Option<&str>,
    cx: &Context<MainShell>,
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
                    .on_click(cx.listener(move |shell, _, _, cx| shell.settings_modal.set_storage_retention(days, cx)))
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
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                if let Some(days) = retention_days {
                                    shell.settings_modal.preview_storage_cleanup(days, active_history_id.clone(), cx);
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
    cx: &Context<MainShell>,
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
    cx: &Context<MainShell>,
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
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell
                                .settings_modal
                                .choose_storage_directory(data_directory, cx)
                        })),
                )
                .when(custom, |row| {
                    row.child(
                        Button::new(format!("{id}-restore"))
                            .ghost()
                            .small()
                            .label("Restore Default")
                            .disabled(busy)
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell
                                    .settings_modal
                                    .reset_storage_directory(data_directory, cx)
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

fn usage_metrics(usage: &StorageUsageSnapshot, cx: &Context<MainShell>) -> gpui_kit::Div {
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
        (
            LucideIcon::Globe,
            "WebView Cache",
            categories.webview_cache.bytes.unwrap_or(0),
            if categories.webview_cache.path.is_some() {
                "Observable cache directory".into()
            } else {
                "Size unavailable on this platform".into()
            },
        ),
        (
            LucideIcon::HardDrive,
            "Other",
            categories.other.bytes,
            format!("{} files", categories.other.file_count),
        ),
    ];
    div()
        .grid()
        .grid_cols(2)
        .gap_2()
        .children(metrics.into_iter().map(|(icon, title, bytes, detail)| {
            div()
                .flex()
                .items_start()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(cx.theme().group_box)
                .child(
                    Icon::new(icon)
                        .size(px(17.0))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::MEDIUM)
                                .child(format_bytes(bytes)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(detail),
                        ),
                )
        }))
}

fn sqlite_breakdown(sqlite: &SQLiteUsageSummary, cx: &Context<MainShell>) -> gpui_kit::Div {
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
                (
                    "Analytics DB + WAL/SHM",
                    format_bytes(
                        sqlite.analytics_db_bytes
                            + sqlite.analytics_wal_bytes
                            + sqlite.analytics_shm_bytes,
                    ),
                ),
            ]
            .into_iter()
            .map(|(label, value)| {
                div()
                    .flex()
                    .justify_between()
                    .gap_3()
                    .py_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(label),
                    )
                    .child(div().text_size(px(12.0)).child(value))
            }),
        )
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
