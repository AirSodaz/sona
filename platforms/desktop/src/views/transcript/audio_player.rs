//! Integrated sticky bottom audio player bar.

use super::super::shell::MainShell;
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
    pub is_muted: bool,
    previous_volume: f32,
}

impl AudioPlayerBar {
    pub fn new() -> Self {
        Self {
            is_playing: false,
            current_time: 0.0,
            total_duration: 0.0,
            speed: 1.0,
            volume: 1.0,
            is_muted: false,
            previous_volume: 1.0,
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

    pub fn seek_by(&mut self, delta: f64) {
        self.current_time = (self.current_time + delta).clamp(0.0, self.total_duration.max(0.0));
    }

    pub fn toggle_mute(&mut self) {
        if self.is_muted {
            self.is_muted = false;
            self.volume = self.previous_volume.max(0.01);
        } else {
            self.previous_volume = self.volume.max(0.01);
            self.is_muted = true;
            self.volume = 0.0;
        }
    }

    pub fn adjust_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.is_muted = false;
            self.previous_volume = self.volume;
        }
    }

    fn format_time(seconds: f64) -> String {
        let mins = (seconds / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
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
                            .label(if self.is_playing { "Pause" } else { "Play" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.toggle_play();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-player-back")
                            .ghost()
                            .small()
                            .label("-5s")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.seek_by(-5.0);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-player-forward")
                            .ghost()
                            .small()
                            .label("+5s")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.seek_by(5.0);
                                cx.notify();
                            })),
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
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("btn-player-speed")
                            .ghost()
                            .small()
                            .label(format!("{:.2}x", self.speed))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.cycle_speed();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-player-mute")
                            .ghost()
                            .small()
                            .label(if self.is_muted { "Unmute" } else { "Mute" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.toggle_mute();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-player-volume-down")
                            .ghost()
                            .small()
                            .label("Vol-")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.adjust_volume(-0.1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("btn-player-volume-up")
                            .ghost()
                            .small()
                            .label(format!("{}%", (self.volume * 100.0).round() as u32))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.adjust_volume(0.1);
                                cx.notify();
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::AudioPlayerBar;

    #[test]
    fn seek_is_clamped_to_duration() {
        let mut player = AudioPlayerBar::new();
        player.total_duration = 10.0;
        player.seek_by(20.0);
        assert_eq!(player.current_time, 10.0);
        player.seek_by(-20.0);
        assert_eq!(player.current_time, 0.0);
    }

    #[test]
    fn mute_round_trip_restores_previous_volume() {
        let mut player = AudioPlayerBar::new();
        player.volume = 0.65;
        player.toggle_mute();
        assert!(player.is_muted);
        assert_eq!(player.volume, 0.0);
        player.toggle_mute();
        assert!(!player.is_muted);
        assert!((player.volume - 0.65).abs() < f32::EPSILON);
    }
}
