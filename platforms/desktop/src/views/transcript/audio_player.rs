//! Integrated sticky bottom audio player bar.
//! Strict 1:1 parity with React `AudioPlayer.tsx`.

use super::super::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
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
        let speeds = [0.5, 0.8, 1.0, 1.25, 1.5, 2.0, 3.0];
        let current_idx = speeds
            .iter()
            .position(|&s| (s - self.speed).abs() < 0.05)
            .unwrap_or(2);
        self.speed = speeds[(current_idx + 1) % speeds.len()];
    }

    pub fn seek_by(&mut self, delta: f64) {
        self.current_time = (self.current_time + delta).clamp(0.0, self.total_duration.max(0.0));
    }

    pub fn seek_to_ratio(&mut self, ratio: f32) {
        let r = ratio.clamp(0.0, 1.0) as f64;
        self.current_time = r * self.total_duration.max(0.0);
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

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
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
            // Left: Play/Pause icon button
            .child(
                div().flex().items_center().child(
                    div()
                        .id("btn-player-play-pause")
                        .flex()
                        .size(px(36.0))
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().secondary))
                        .child(
                            Icon::new(if self.is_playing {
                                LucideIcon::Pause
                            } else {
                                LucideIcon::Play
                            })
                            .size(px(18.0)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workbench.player.toggle_play();
                            cx.notify();
                        })),
                ),
            )
            // Center: Timeline (Current Time, Scrubber Slider Track, Duration)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_1()
                    .mx_6()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(".AppleSystemUIFont")
                            .text_color(cx.theme().muted_foreground)
                            .child(Self::format_time(self.current_time)),
                    )
                    .child(
                        div()
                            .id("player-scrubber-track")
                            .relative()
                            .flex()
                            .items_center()
                            .flex_1()
                            .h(px(16.0))
                            .cursor_pointer()
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(cx.theme().secondary)
                                    .child(
                                        div()
                                            .w(relative(progress))
                                            .h_full()
                                            .rounded_full()
                                            .bg(cx.theme().primary),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(-4.0))
                                            .left(relative(progress))
                                            .size(px(12.0))
                                            .rounded_full()
                                            .bg(cx.theme().primary)
                                            .shadow_sm(),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(".AppleSystemUIFont")
                            .text_color(cx.theme().muted_foreground)
                            .child(Self::format_time(self.total_duration)),
                    ),
            )
            // Right: Speed, Mute/Unmute Icon, and Volume Slider Track
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("btn-player-speed")
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(28.0))
                            .px_2()
                            .rounded_sm()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .child(
                                format!("{:.2}x", self.speed)
                                    .replace(".00x", "x")
                                    .replace("0x", "x"),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.cycle_speed();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("btn-player-mute")
                            .flex()
                            .size(px(32.0))
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .child(
                                Icon::new(if self.is_muted || self.volume <= 0.0 {
                                    LucideIcon::VolumeX
                                } else {
                                    LucideIcon::Volume2
                                })
                                .size(px(16.0)),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.player.toggle_mute();
                                cx.notify();
                            })),
                    )
                    // Volume slider track
                    .child(
                        div()
                            .id("player-volume-track")
                            .relative()
                            .flex()
                            .items_center()
                            .w(px(64.0))
                            .h(px(16.0))
                            .cursor_pointer()
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(cx.theme().secondary)
                                    .child(
                                        div()
                                            .w(relative(self.volume))
                                            .h_full()
                                            .rounded_full()
                                            .bg(cx.theme().primary),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(-3.0))
                                            .left(relative(self.volume))
                                            .size(px(10.0))
                                            .rounded_full()
                                            .bg(cx.theme().primary),
                                    ),
                            ),
                    ),
            )
    }
}

impl Default for AudioPlayerBar {
    fn default() -> Self {
        Self::new()
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
