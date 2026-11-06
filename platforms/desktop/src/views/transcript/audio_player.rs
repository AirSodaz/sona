//! Integrated sticky bottom audio player bar.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

pub struct AudioPlayerBar {
    pub is_playing: bool,
    pub current_time: f64,
    pub total_duration: f64,
    pub speed: f32,
    pub volume: f32,
}

impl AudioPlayerBar {
    pub fn new() -> Self {
        Self {
            is_playing: false,
            current_time: 124.5,
            total_duration: 380.0,
            speed: 1.0,
            volume: 1.0,
        }
    }

    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
    }

    pub fn cycle_speed(&mut self) {
        self.speed = match self.speed {
            s if (s - 1.0).abs() < 0.05 => 1.25,
            s if (s - 1.25).abs() < 0.05 => 1.5,
            s if (s - 1.5).abs() < 0.05 => 2.0,
            s if (s - 2.0).abs() < 0.05 => 0.75,
            _ => 1.0,
        };
    }

    fn format_time(seconds: f64) -> String {
        let mins = (seconds / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        let progress = if self.total_duration > 0.0 {
            (self.current_time / self.total_duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(56.0))
            .w_full()
            .px_4()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                // Play/pause and timing
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("btn-player-toggle")
                            .primary()
                            .small()
                            .label(if self.is_playing { "Pause" } else { "Play" }),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().foreground)
                            .child(format!(
                                "{} / {}",
                                Self::format_time(self.current_time),
                                Self::format_time(self.total_duration)
                            )),
                    ),
            )
            .child(
                // Progress scrubber bar
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .mx_6()
                    .h(px(6.0))
                    .rounded_full()
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .w(relative(progress))
                            .h_full()
                            .rounded_full()
                            .bg(cx.theme().primary),
                    ),
            )
            .child(
                // Speed & Volume
                div().flex().items_center().gap_2().child(
                    Button::new("btn-player-speed")
                        .ghost()
                        .small()
                        .label(format!("{:.2}x", self.speed)),
                ),
            )
    }
}
