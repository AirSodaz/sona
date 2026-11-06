//! Individual transcript segment row with speaker badge, timestamp seek, and text.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[derive(Clone, Debug)]
pub struct SegmentData {
    pub id: String,
    pub start_time: f64,
    pub end_time: f64,
    pub speaker: String,
    pub text: String,
    pub translation: Option<String>,
}

pub struct SegmentRow;

impl SegmentRow {
    fn format_timestamp(seconds: f64) -> String {
        let mins = (seconds / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        let millis = ((seconds.fract()) * 10.0).floor() as u64;
        format!("{:02}:{:02}.{}", mins, secs, millis)
    }

    pub fn render(data: &SegmentData, is_active: bool, cx: &App) -> impl IntoElement {
        let timestamp_str = Self::format_timestamp(data.start_time);

        div()
            .id(ElementId::from(format!("seg-row-{}", data.id)))
            .flex()
            .flex_col()
            .w_full()
            .p_3()
            .rounded_md()
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .gap_1p5()
            .child(
                // Metadata row: timestamp seek button + speaker badge
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new(format!("btn-seek-{}", data.id))
                            .ghost()
                            .xsmall()
                            .label(timestamp_str),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(cx.theme().secondary)
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(data.speaker.clone()),
                    ),
            )
            .child(
                // Segment transcript text
                div()
                    .text_size(px(14.0))
                    .line_height(relative(1.5))
                    .text_color(cx.theme().foreground)
                    .child(data.text.clone()),
            )
            .when_some(data.translation.clone(), |this, trans| {
                this.child(
                    div()
                        .text_size(px(13.0))
                        .line_height(relative(1.4))
                        .text_color(cx.theme().muted)
                        .child(trans),
                )
            })
    }
}
