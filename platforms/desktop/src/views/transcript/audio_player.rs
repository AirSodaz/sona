//! Integrated sticky bottom audio player bar.
//! Strict 1:1 parity with React `AudioPlayer.tsx`.

use super::workbench::TranscriptWorkbench;
use crate::platform::audio_playback::{
    AudioPlaybackController, AudioPlaybackEvent, AudioPlaybackEvents,
};
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::slider::{Slider, SliderState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::path::PathBuf;

pub struct AudioPlayerBar {
    pub is_playing: bool,
    pub current_time: f64,
    pub total_duration: f64,
    pub speed: f32,
    pub volume: f32,
    pub is_muted: bool,
    previous_volume: f32,
    playback: AudioPlaybackController,
    audio_loaded: bool,
    pub audio_error: Option<String>,
    seek_slider: Entity<SliderState>,
    volume_slider: Entity<SliderState>,
}

impl AudioPlayerBar {
    pub fn new(cx: &mut Context<TranscriptWorkbench>) -> Self {
        Self {
            is_playing: false,
            current_time: 0.0,
            total_duration: 0.0,
            speed: 1.0,
            volume: 1.0,
            is_muted: false,
            previous_volume: 1.0,
            playback: AudioPlaybackController::new(),
            audio_loaded: false,
            audio_error: None,
            seek_slider: cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001)),
            volume_slider: cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(1.0)
                    .step(0.01)
                    .default_value(1.0)
            }),
        }
    }

    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
        self.playback.set_playing(self.is_playing);
    }

    pub fn is_visible(&self) -> bool {
        self.audio_loaded || self.audio_error.is_some()
    }

    pub fn events(&self) -> AudioPlaybackEvents {
        self.playback.events()
    }

    pub fn load(&mut self, path: PathBuf) {
        self.current_time = 0.0;
        self.is_playing = false;
        self.audio_loaded = true;
        self.audio_error = None;
        self.playback.set_volume(self.volume);
        self.playback.set_speed(self.speed);
        self.playback.load(path);
    }

    pub fn reset(&mut self) {
        self.playback.stop();
        self.current_time = 0.0;
        self.total_duration = 0.0;
        self.is_playing = false;
        self.audio_loaded = false;
        self.audio_error = None;
    }

    pub fn apply_event(&mut self, event: AudioPlaybackEvent) -> Option<(bool, f64)> {
        match event {
            AudioPlaybackEvent::State {
                position,
                duration,
                is_playing,
            } => {
                self.current_time = position;
                self.total_duration = duration;
                self.is_playing = is_playing;
                Some((is_playing, position))
            }
            AudioPlaybackEvent::Ended => {
                self.is_playing = false;
                self.current_time = self.total_duration;
                Some((false, self.current_time))
            }
            AudioPlaybackEvent::Error(error) => {
                self.is_playing = false;
                self.audio_error = Some(error);
                None
            }
        }
    }

    pub fn cycle_speed(&mut self) {
        let speeds = [0.5, 0.8, 1.0, 1.25, 1.5, 2.0, 3.0];
        let current_idx = speeds
            .iter()
            .position(|&s| (s - self.speed).abs() < 0.05)
            .unwrap_or(2);
        self.speed = speeds[(current_idx + 1) % speeds.len()];
        self.playback.set_speed(self.speed);
    }

    pub fn seek_by(&mut self, delta: f64) {
        self.current_time = (self.current_time + delta).clamp(0.0, self.total_duration.max(0.0));
        self.playback.seek(self.current_time);
    }

    pub fn seek_to(&mut self, seconds: f64) {
        self.current_time = seconds.clamp(0.0, self.total_duration.max(0.0));
        self.playback.seek(self.current_time);
    }

    pub fn seek_to_ratio(&mut self, ratio: f32) {
        let r = ratio.clamp(0.0, 1.0) as f64;
        self.current_time = r * self.total_duration.max(0.0);
        self.playback.seek(self.current_time);
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
        self.playback.set_volume(self.volume);
    }

    pub fn adjust_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.is_muted = false;
            self.previous_volume = self.volume;
        }
        self.playback.set_volume(self.volume);
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.is_muted = false;
            self.previous_volume = self.volume;
        }
        self.playback.set_volume(self.volume);
    }

    pub fn sync_seek_slider(&self, window: &mut Window, cx: &mut Context<TranscriptWorkbench>) {
        let ratio = if self.total_duration > 0.0 {
            (self.current_time / self.total_duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        self.seek_slider
            .update(cx, |state, cx| state.set_value(ratio, window, cx));
    }

    pub fn sync_volume_slider(&self, window: &mut Window, cx: &mut Context<TranscriptWorkbench>) {
        self.volume_slider
            .update(cx, |state, cx| state.set_value(self.volume, window, cx));
    }

    pub fn seek_slider(&self) -> &Entity<SliderState> {
        &self.seek_slider
    }

    pub fn volume_slider(&self) -> &Entity<SliderState> {
        &self.volume_slider
    }

    fn format_time(seconds: f64) -> String {
        let mins = (seconds / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn render(&mut self, cx: &mut Context<TranscriptWorkbench>) -> impl IntoElement {
        if !self.audio_loaded {
            return div().h_0().into_any_element();
        }

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
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
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
                                this.player.toggle_play();
                                this.editor.update_playback(
                                    this.player.is_playing,
                                    false,
                                    this.player.current_time,
                                );
                                cx.notify();
                            })),
                    )
                    .when_some(self.audio_error.clone(), |this, error| {
                        this.child(
                            div()
                                .max_w(px(300.0))
                                .text_size(px(11.0))
                                .text_color(cx.theme().danger)
                                .child(error),
                        )
                    }),
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
                        Slider::new(&self.seek_slider)
                            .disabled(self.total_duration <= 0.0)
                            .accessibility_label("Seek")
                            .flex_1()
                            .h(px(16.0)),
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
                                this.player.cycle_speed();
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
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.player.toggle_mute();
                                this.player.sync_volume_slider(window, cx);
                                cx.notify();
                            })),
                    )
                    // Volume slider track
                    .child(
                        Slider::new(&self.volume_slider)
                            .accessibility_label("Volume")
                            .w(px(64.0))
                            .h(px(16.0)),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::AudioPlayerBar;

    #[test]
    fn formats_audio_time_in_minutes_and_seconds() {
        assert_eq!(AudioPlayerBar::format_time(61.9), "01:01");
        assert_eq!(AudioPlayerBar::format_time(0.0), "00:00");
    }
}
