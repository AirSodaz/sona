//! Onboarding wizard modal for first-run setup and model downloads.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct OnboardingWizard {
    pub is_open: bool,
    pub step: usize,
}

impl OnboardingWizard {
    pub fn new() -> Self {
        Self {
            is_open: false,
            step: 0,
        }
    }

    pub fn next_step(&mut self) {
        if self.step < 2 {
            self.step += 1;
        } else {
            self.is_open = false;
        }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .when(self.is_open, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .bg(hsla(0.0, 0.0, 0.0, 0.6))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(520.0))
                                .p_8()
                                .rounded_2xl()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .gap_6()
                                .child(
                                    div()
                                        .text_size(px(24.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Welcome to Sona Desktop"),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(cx.theme().muted)
                                        .child("Privacy-first offline speech recognition, bilingual subtitling, and AI transcript workbench powered by pure Rust."),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(cx.theme().muted)
                                                .child(format!("Step {} of 3", self.step + 1)),
                                        )
                                        .child(
                                            Button::new("btn-onboarding-next")
                                                .primary()
                                                .label(if self.step < 2 { "Continue" } else { "Get Started" }),
                                        ),
                                ),
                        ),
                )
            })
    }
}
