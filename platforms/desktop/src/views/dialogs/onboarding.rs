//! Onboarding wizard modal for first-run setup and model downloads.
//! Strict 1:1 parity with React `FirstRunGuide.tsx`.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::tokens::{COLOR_PROJECT_BLUE, RADIUS_FULL, RADIUS_LG, RADIUS_MD, RADIUS_SM};

pub struct OnboardingWizard {
    pub is_open: bool,
    pub step: usize, // 0: Models, 1: Microphone
    pub sensevoice_downloading: bool,
    pub sensevoice_progress: f32,
    pub whisper_downloading: bool,
    pub whisper_progress: f32,
}

impl OnboardingWizard {
    pub fn new() -> Self {
        Self {
            is_open: false,
            step: 0,
            sensevoice_downloading: false,
            sensevoice_progress: 100.0,
            whisper_downloading: false,
            whisper_progress: 100.0,
        }
    }

    pub fn next_step(&mut self) {
        if self.step == 0 {
            self.step = 1;
        } else {
            self.is_open = false;
        }
    }

    pub fn prev_step(&mut self) {
        if self.step > 0 {
            self.step -= 1;
        }
    }

    pub fn render(&mut self, cx: &mut Context<super::super::shell::MainShell>) -> impl IntoElement {
        let step = self.step;

        div().when(self.is_open, |this| {
            this.child(
                div()
                    .id("welcome-screen-backdrop")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(hsla(0.0, 0.0, 0.0, 0.65))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.onboarding.is_open = false;
                        cx.notify();
                    }))
                    .child(
                        // Main Split Container: 820px width, 540px height
                        div()
                            .id("welcome-screen-card")
                            .flex()
                            .w(px(840.0))
                            .h(px(540.0))
                            .rounded(RADIUS_LG)
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_2xl()
                            .overflow_hidden()
                            .on_click(|_, _, cx| cx.stop_propagation())
                            // Left: Brand Hero Panel (280px width)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .w(px(280.0))
                                    .h_full()
                                    .p_8()
                                    .bg(cx.theme().secondary)
                                    .border_r_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .child(
                                                div()
                                                    .size(px(48.0))
                                                    .rounded(RADIUS_MD)
                                                    .bg(COLOR_PROJECT_BLUE.opacity(0.15))
                                                    .text_color(COLOR_PROJECT_BLUE)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(Icon::new(LucideIcon::SquarePen).size(px(28.0))),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(22.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child("Welcome to Sona"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .line_height(relative(1.45))
                                                    .text_color(cx.theme().muted)
                                                    .child(
                                                        "Privacy-first offline speech recognition, bilingual subtitling, and AI transcript workbench powered by pure Rust.",
                                                    ),
                                            ),
                                    )
                                    // S-curve subtle motif
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(cx.theme().muted)
                                            .child("Sona Desktop v0.10.3"),
                                    ),
                            )
                            // Right: Content Panel
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .h_full()
                                    .p_8()
                                    .justify_between()
                                    // Header: Stepper Nav
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_6()
                                            .pb_4()
                                            .border_b_1()
                                            .border_color(cx.theme().border)
                                            // Step 1 Nav Item
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .size(px(22.0))
                                                            .rounded_full()
                                                            .bg(if step == 0 {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().success
                                                            })
                                                            .text_color(cx.theme().background)
                                                            .text_size(px(11.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(if step > 0 {
                                                                Icon::new(LucideIcon::Check).size(px(12.0))
                                                            } else {
                                                                Icon::new(LucideIcon::Box).size(px(12.0))
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .font_weight(if step == 0 {
                                                                FontWeight::SEMIBOLD
                                                            } else {
                                                                FontWeight::NORMAL
                                                            })
                                                            .text_color(if step == 0 {
                                                                cx.theme().foreground
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .child("1. Models"),
                                                    ),
                                            )
                                            // Step 2 Nav Item
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .size(px(22.0))
                                                            .rounded_full()
                                                            .bg(if step == 1 {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .text_color(cx.theme().background)
                                                            .text_size(px(11.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(Icon::new(LucideIcon::Mic).size(px(12.0))),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .font_weight(if step == 1 {
                                                                FontWeight::SEMIBOLD
                                                            } else {
                                                                FontWeight::NORMAL
                                                            })
                                                            .text_color(if step == 1 {
                                                                cx.theme().foreground
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .child("2. Microphone"),
                                                    ),
                                            ),
                                    )
                                    // Body: Step Content
                                    .child(
                                        div()
                                            .id("welcome-body-scroll")
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .py_4()
                                            .gap_4()
                                            .overflow_y_scroll()
                                            .when(step == 0, |this| {
                                                this.child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .text_size(px(16.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(cx.theme().foreground)
                                                                .child("Recommended Offline Models"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(13.0))
                                                                .text_color(cx.theme().muted)
                                                                .child(
                                                                    "Download high-accuracy models to transcribe audio completely on-device without cloud latency.",
                                                                ),
                                                        ),
                                                )
                                                // Model 1: SenseVoice Small
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .p_3()
                                                        .gap_2()
                                                        .rounded(RADIUS_MD)
                                                        .border_1()
                                                        .border_color(cx.theme().border)
                                                        .bg(cx.theme().secondary)
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(
                                                                    div()
                                                                        .text_size(px(14.0))
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .child("SenseVoice Small"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(
                                                                            div()
                                                                                .px_1p5()
                                                                                .py(px(1.0))
                                                                                .rounded(RADIUS_SM)
                                                                                .bg(cx.theme().background)
                                                                                .border_1()
                                                                                .border_color(cx.theme().border)
                                                                                .text_size(px(11.0))
                                                                                .child("230 MB"),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .px_1p5()
                                                                                .py(px(1.0))
                                                                                .rounded(RADIUS_SM)
                                                                                .bg(COLOR_PROJECT_BLUE.opacity(0.12))
                                                                                .text_color(COLOR_PROJECT_BLUE)
                                                                                .text_size(px(11.0))
                                                                                .child("ZH, EN, JA, KO, YUE"),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .text_color(cx.theme().muted)
                                                                .child(
                                                                    "Ultra-fast bilingual speech recognition optimized for low latency and rich audio events.",
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .text_size(px(11.0))
                                                                .child(div().text_color(cx.theme().success).child("● Ready to use"))
                                                                .child(div().text_color(cx.theme().muted).child("100%")),
                                                        ),
                                                )
                                                // Model 2: Whisper Turbo
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .p_3()
                                                        .gap_2()
                                                        .rounded(RADIUS_MD)
                                                        .border_1()
                                                        .border_color(cx.theme().border)
                                                        .bg(cx.theme().secondary)
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(
                                                                    div()
                                                                        .text_size(px(14.0))
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .child("Whisper Turbo"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(
                                                                            div()
                                                                                .px_1p5()
                                                                                .py(px(1.0))
                                                                                .rounded(RADIUS_SM)
                                                                                .bg(cx.theme().background)
                                                                                .border_1()
                                                                                .border_color(cx.theme().border)
                                                                                .text_size(px(11.0))
                                                                                .child("1.6 GB"),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .px_1p5()
                                                                                .py(px(1.0))
                                                                                .rounded(RADIUS_SM)
                                                                                .bg(COLOR_PROJECT_BLUE.opacity(0.12))
                                                                                .text_color(COLOR_PROJECT_BLUE)
                                                                                .text_size(px(11.0))
                                                                                .child("Multilingual (99+)"),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .text_color(cx.theme().muted)
                                                                .child(
                                                                    "State-of-the-art multilingual model with superior accuracy for complex accents and mixed speech.",
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .text_size(px(11.0))
                                                                .child(div().text_color(cx.theme().success).child("● Ready to use"))
                                                                .child(div().text_color(cx.theme().muted).child("100%")),
                                                        ),
                                                )
                                            })
                                            .when(step == 1, |this| {
                                                this.child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .text_size(px(16.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(cx.theme().foreground)
                                                                .child("Microphone & Audio Input"),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(13.0))
                                                                .text_color(cx.theme().muted)
                                                                .child(
                                                                    "Select your primary recording device for live voice typing and real-time transcription.",
                                                                ),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .p_4()
                                                        .rounded(RADIUS_MD)
                                                        .border_1()
                                                        .border_color(cx.theme().border)
                                                        .bg(cx.theme().secondary)
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_3()
                                                                .child(
                                                                    Icon::new(LucideIcon::Mic)
                                                                        .size(px(20.0))
                                                                        .text_color(cx.theme().primary),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .flex_col()
                                                                        .child(
                                                                            div()
                                                                                .text_size(px(14.0))
                                                                                .font_weight(FontWeight::MEDIUM)
                                                                                .child("Default System Microphone"),
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .text_size(px(12.0))
                                                                                .text_color(cx.theme().muted)
                                                                                .child("Auto-routed by Windows CoreAudio"),
                                                                        ),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1p5()
                                                                .px_2()
                                                                .py_1()
                                                                .rounded(RADIUS_FULL)
                                                                .bg(cx.theme().success.opacity(0.12))
                                                                .text_color(cx.theme().success)
                                                                .text_size(px(11.0))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .child(Icon::new(LucideIcon::Check).size(px(12.0)))
                                                                .child("Access Granted"),
                                                        ),
                                                )
                                            }),
                                    )
                                    // Footer Actions: Later (left), Back, Primary
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .pt_4()
                                            .border_t_1()
                                            .border_color(cx.theme().border)
                                            .child(
                                                Button::new("btn-onboarding-later")
                                                    .ghost()
                                                    .label("Later")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.onboarding.is_open = false;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .when(step > 0, |actions| {
                                                        actions.child(
                                                            Button::new("btn-onboarding-back")
                                                                .ghost()
                                                                .label("Back")
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.onboarding.prev_step();
                                                                    cx.notify();
                                                                })),
                                                        )
                                                    })
                                                    .child(
                                                        Button::new("btn-onboarding-primary")
                                                            .primary()
                                                            .label(if step == 0 {
                                                                "Continue"
                                                            } else {
                                                                "Finish"
                                                            })
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.onboarding.next_step();
                                                                cx.notify();
                                                            })),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
        })
    }
}

impl Default for OnboardingWizard {
    fn default() -> Self {
        Self::new()
    }
}
