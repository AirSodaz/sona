use gpui_kit::component::button::Button;
use gpui_kit::component::chart::{AreaChart, BarChart};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_application::dashboard::DashboardSnapshotTime;
use sona_core::dashboard::models::{
    ContentTrendPoint, DashboardSnapshotDomainModel, UsageBreakdown, UsageTrendPoint,
};

use crate::views::shell::MainShell;

#[derive(Clone, Copy)]
struct DashboardColors {
    background: Hsla,
    input: Hsla,
    border: Hsla,
    muted: Hsla,
    danger: Hsla,
    accent: Hsla,
}

pub(super) fn dashboard_snapshot_time() -> DashboardSnapshotTime {
    let now = chrono::Local::now();
    DashboardSnapshotTime {
        generated_at: now.to_rfc3339(),
        today: now.date_naive(),
        local_utc_offset_seconds: now.offset().local_minus_utc(),
    }
}

fn metric(label: &str, value: &str, detail: Option<&str>, colors: DashboardColors) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .min_w_0()
        .p_4()
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.background)
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors.muted)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(23.0))
                .font_weight(FontWeight::BOLD)
                .child(value.to_string()),
        )
        .when_some(detail, |card, detail| {
            card.child(
                div()
                    .text_size(px(11.0))
                    .text_color(colors.muted)
                    .child(detail.to_string()),
            )
        })
        .into_any_element()
}

fn trend_chart<T>(
    id: &'static str,
    data: Vec<T>,
    band: impl Fn(&T) -> String + 'static,
    value: impl Fn(&T) -> f64 + 'static,
    colors: DashboardColors,
) -> AnyElement
where
    T: 'static,
{
    div()
        .h(px(104.0))
        .child(
            AreaChart::new(data)
                .id(id)
                .x(band)
                .y(value)
                .stroke(colors.accent)
                .fill(colors.accent.opacity(0.12))
                .x_axis(false)
                .grid(false),
        )
        .into_any_element()
}

fn breakdown_chart(
    id: &'static str,
    data: Vec<UsageBreakdown>,
    _colors: DashboardColors,
) -> AnyElement {
    div()
        .h(px(132.0))
        .child(
            BarChart::new(data)
                .id(id)
                .band(|row| row.label.clone())
                .value(|row| row.value as f64)
                .band_tick_count(4)
                .value_axis(false)
                .grid(false),
        )
        .into_any_element()
}

fn two_columns(left: AnyElement, right: AnyElement) -> AnyElement {
    div()
        .flex()
        .gap_3()
        .child(div().flex_1().min_w_0().child(left))
        .child(div().flex_1().min_w_0().child(right))
        .into_any_element()
}

fn section(
    title: &str,
    description: &str,
    content: AnyElement,
    colors: DashboardColors,
) -> AnyElement {
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
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child(description.to_string()),
                ),
        )
        .child(content)
        .into_any_element()
}

pub(super) fn dashboard_panel(
    snapshot: Option<&DashboardSnapshotDomainModel>,
    is_fast_loading: bool,
    is_deep_loading: bool,
    error: Option<&str>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let colors = DashboardColors {
        background: cx.theme().background,
        input: cx.theme().input,
        border: cx.theme().border,
        muted: cx.theme().muted,
        danger: cx.theme().danger,
        accent: cx.theme().primary,
    };
    if snapshot.is_none() {
        let state = if is_fast_loading {
            div()
                .flex()
                .items_center()
                .gap_2()
                .p_4()
                .rounded_lg()
                .bg(colors.input)
                .text_color(colors.muted)
                .child(Spinner::new())
                .child("Loading dashboard...")
                .into_any_element()
        } else if let Some(error) = error {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(colors.danger)
                .bg(colors.input)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Dashboard unavailable"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(colors.muted)
                                .child(error.to_string()),
                        ),
                )
                .child(
                    Button::new("dashboard-retry")
                        .outline()
                        .small()
                        .label("Retry")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.settings_modal.retry_dashboard(cx);
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        return div()
            .id("settings-dashboard-state")
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .child(state)
            .into_any_element();
    }

    let mut body = div()
        .id("settings-dashboard-scroll")
        .flex()
        .flex_col()
        .gap_5()
        .size_full()
        .overflow_y_scroll();

    body = body.child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .pb_3()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Dashboard"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(colors.muted)
                    .child("Review global content and LLM usage without leaving Settings."),
            ),
    );

    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => return div().size_full().into_any_element(),
    };

    let overview = &snapshot.content.overview;
    let llm = &snapshot.llm_usage;
    if overview.item_count == 0 && llm.totals.call_count == 0 {
        return body
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_5()
                    .rounded_xl()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.input)
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("No dashboard data yet"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(colors.muted)
                            .child("Saved transcripts and successful LLM calls will start appearing here automatically."),
                    ),
            )
            .into_any_element();
    }

    let content_trends = overview.recent_daily_items.clone();
    let content_duration = content_trends.clone();
    let llm_calls = llm.recent_daily.clone();
    let llm_tokens = llm_calls.clone();

    let items_detail = format!(
        "{} recordings · {} batch imports",
        overview.recording_count_display, overview.batch_count_display
    );
    let tag_detail = format!(
        "{} tagged · {} untagged",
        overview.tagged_count_display, overview.untagged_count_display
    );
    let global_content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(two_columns(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .child(metric(
                    "Items",
                    &overview.item_count_display,
                    Some(&items_detail),
                    colors,
                ))
                .child(trend_chart(
                    "dashboard-items-trend",
                    content_trends,
                    |point: &ContentTrendPoint| point.date_label.clone(),
                    |point| point.item_count as f64,
                    colors,
                ))
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .child(metric(
                    "Total Duration",
                    &overview.total_duration_display,
                    None,
                    colors,
                ))
                .child(trend_chart(
                    "dashboard-duration-trend",
                    content_duration,
                    |point: &ContentTrendPoint| point.date_label.clone(),
                    |point| point.duration_seconds,
                    colors,
                ))
                .into_any_element(),
        ))
        .child(two_columns(
            metric(
                "Transcript Characters",
                overview
                    .transcript_character_count_display
                    .as_deref()
                    .unwrap_or(if is_deep_loading {
                        "Scanning..."
                    } else {
                        "..."
                    }),
                Some("Across saved transcripts"),
                colors,
            ),
            metric(
                "Tags",
                &overview.tag_count_display,
                Some(&tag_detail),
                colors,
            ),
        ));
    body = body.child(section(
        "Global Content",
        "Saved content volume, where it lives, and speaker attribution across transcripts.",
        global_content.into_any_element(),
        colors,
    ));

    if let Some(speakers) = snapshot.content.speakers.as_ref() {
        let speaker_metrics = two_columns(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(metric(
                    "Speaker-Annotated Items",
                    &speakers.annotated_item_count_display,
                    None,
                    colors,
                ))
                .child(metric(
                    "Identified Speakers",
                    &speakers.identified_speaker_count_display,
                    None,
                    colors,
                ))
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(metric(
                    "Speaker-Attributed Duration",
                    &speakers.speaker_attributed_duration_display,
                    None,
                    colors,
                ))
                .child(metric(
                    "Anonymous Speaker Slots",
                    &speakers.anonymous_speaker_slot_count_display,
                    None,
                    colors,
                ))
                .into_any_element(),
        );
        let speaker_rows =
            speakers.top_identified_speaker_rows.iter().fold(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.background)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Top Identified Speakers"),
                    ),
                |rows, speaker| {
                    rows.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .py_1()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(speaker.label.clone())
                            .child(div().text_size(px(11.0)).text_color(colors.muted).child(
                                format!(
                                    "{} · {} segments · {} items",
                                    speaker.duration_display,
                                    speaker.segment_count_display,
                                    speaker.item_count_display
                                ),
                            )),
                    )
                },
            );
        let speaker_summary = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(speaker_metrics)
            .child(two_columns(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.background)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Coverage"))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_3()
                            .child("Segments attributed")
                            .child(speakers.segment_coverage_label.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_3()
                            .child("Duration attributed")
                            .child(speakers.duration_coverage_label.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_3()
                            .child("Total segments")
                            .child(speakers.total_segment_count_display.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_3()
                            .child("Total segment duration")
                            .child(speakers.total_segment_duration_display.clone()),
                    )
                    .into_any_element(),
                speaker_rows
                    .when(speakers.top_identified_speaker_rows.is_empty(), |rows| {
                        rows.child(
                            div()
                                .text_color(colors.muted)
                                .child("No identified speakers yet."),
                        )
                    })
                    .into_any_element(),
            ));
        body = body.child(section(
            "Speaker Insights",
            "Speaker attribution and identified speaker activity across saved transcripts.",
            speaker_summary.into_any_element(),
            colors,
        ));
    } else if is_deep_loading {
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .p_3()
                .rounded_lg()
                .bg(colors.input)
                .text_color(colors.muted)
                .child(Spinner::new())
                .child("Scanning transcript and speaker details..."),
        );
    }

    let usage_feature = two_columns(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .child(metric(
                "Successful Calls",
                &llm.totals.call_count_display,
                None,
                colors,
            ))
            .child(trend_chart(
                "dashboard-llm-calls-trend",
                llm_calls,
                |point: &UsageTrendPoint| point.date_label.clone(),
                |point| point.stats.call_count as f64,
                colors,
            ))
            .into_any_element(),
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .child(metric(
                "Total Tokens",
                &llm.totals.total_tokens_display,
                None,
                colors,
            ))
            .child(trend_chart(
                "dashboard-llm-tokens-trend",
                llm_tokens,
                |point: &UsageTrendPoint| point.date_label.clone(),
                |point| point.stats.total_tokens as f64,
                colors,
            ))
            .into_any_element(),
    );
    let usage_counts = two_columns(
        metric(
            "Calls With Usage",
            &llm.totals.calls_with_usage_display,
            None,
            colors,
        ),
        metric(
            "Calls Missing Usage",
            &llm.totals.calls_without_usage_display,
            None,
            colors,
        ),
    );
    let mut usage = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(usage_feature)
        .child(usage_counts)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child(format!(
                    "{} prompt / {} completion",
                    llm.totals.prompt_tokens_display, llm.totals.completion_tokens_display
                ))
                .child(llm.tracking_since_display.as_ref().map_or_else(
                    || "No tracked calls yet.".to_string(),
                    |date| format!("Tracking since {date}"),
                )),
        )
        .child(two_columns(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .child(div().font_weight(FontWeight::SEMIBOLD).child("By Provider"))
                .child(breakdown_chart(
                    "dashboard-provider-breakdown",
                    llm.by_provider_top_rows.clone(),
                    colors,
                ))
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .child(div().font_weight(FontWeight::SEMIBOLD).child("By Category"))
                .child(breakdown_chart(
                    "dashboard-category-breakdown",
                    llm.by_category_top_rows.clone(),
                    colors,
                ))
                .into_any_element(),
        ));
    if let Some(error) = error {
        usage = usage.child(
            div()
                .p_3()
                .rounded_md()
                .bg(colors.input)
                .text_color(colors.muted)
                .child(format!(
                    "Some dashboard details could not be refreshed: {error}"
                )),
        );
    }
    body.child(section(
        "LLM Usage",
        "Successful provider calls and token usage recorded by the application.",
        usage.into_any_element(),
        colors,
    ))
    .into_any_element()
}
