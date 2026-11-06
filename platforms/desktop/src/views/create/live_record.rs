//! Live audio recording panel with real-time waveform visualizer and elapsed timer.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct LiveRecordPanel {
    pub is_recording: bool,
    pub is_paused: bool,
    pub elapsed_seconds: u64,
    pub audio_source: String,
    pub peaks: Vec<f32>,
}

impl LiveRecordPanel {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            is_paused: false,
            elapsed_seconds: 0,
            audio_source: "Default Microphone".to_string(),
            peaks: vec![0.1, 0.25, 0.4, 0.2, 0.6, 0.8, 0.3, 0.5, 0.2, 0.4, 0.7, 0.35],
        }
    }

    pub fn toggle_recording(&mut self) {
        self.is_recording = !self.is_recording;
        if !self.is_recording {
            self.is_paused = false;
            self.elapsed_seconds = 0;
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.is_recording {
            self.is_paused = !self.is_paused;
        }
    }

    fn formatted_time(&self) -> String {
        let hrs = self.elapsed_seconds / 3600;
        let mins = (self.elapsed_seconds % 3600) / 60;
        let secs = self.elapsed_seconds % 60;
        format!("{:02}:{:02}:{:02}", hrs, mins, secs)
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .p_8()
            .gap_6()
            .child(
                // Timer display
                div()
                    .text_size(px(48.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if self.is_recording && !self.is_paused {
                        cx.theme().primary
                    } else {
                        cx.theme().foreground
                    })
                    .child(self.formatted_time()),
            )
            .child(
                // Audio waveform visualizer bar area
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .h(px(64.0))
                    .w(px(320.0))
                    .px_4()
                    .rounded_md()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .children(self.peaks.iter().enumerate().map(|(i, &p)| {
                        let bar_h = (p * 50.0).max(4.0);
                        div()
                            .id(ElementId::from(format!("waveform-bar-{}", i)))
                            .w(px(4.0))
                            .h(px(bar_h))
                            .rounded_full()
                            .bg(if self.is_recording {
                                cx.theme().primary
                            } else {
                                cx.theme().muted
                            })
                    })),
            )
            .child(
                // Controls bar
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(if !self.is_recording {
                        Button::new("btn-record")
                            .primary()
                            .large()
                            .label("Start Recording")
                    } else {
                        Button::new("btn-stop")
                            .danger()
                            .large()
                            .label("Stop & Transcribe")
                    })
                    .when(self.is_recording, |this| {
                        this.child(
                            Button::new("btn-pause")
                                .outline()
                                .large()
                                .label(if self.is_paused { "Resume" } else { "Pause" }),
                        )
                    }),
            )
            .child(
                // Audio source info
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted)
                    .child(format!("Input Source: {}", self.audio_source)),
            )
    }
}
