//! Floating Caption Overlay Window for live bilingual subtitles (Gap G14).
//! Borderless, transparent, click-through, always-on-top window with 4 layout modes,
//! high-contrast text stroke outline, disappearance delay timer, and hover controls.

use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::app::GlobalEventBus;
use crate::event_bus::AppEvent;
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};

/// Window Title constant used for Win32 HWND identification.
pub const CAPTION_WINDOW_TITLE: &str = "Sona Caption Overlay";

/// Layout modes supported by the floating caption overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CaptionLayoutMode {
    /// Single-line bottom caption: displays only the active single utterance.
    #[default]
    SingleLineBottom,
    /// Dual-line bilingual layout: original text + translated text stacked vertically.
    DualLineBilingual,
    /// Multi-line wrapped subtitle stream: displays recent segment history (up to 3 lines).
    MultiLineWrapped,
    /// Compact ticker capsule: minimal floating capsule with speaker badge and compact ticker.
    CompactTicker,
}

impl CaptionLayoutMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::SingleLineBottom => "Single Line",
            Self::DualLineBilingual => "Dual Bilingual",
            Self::MultiLineWrapped => "Multi-line Wrapped",
            Self::CompactTicker => "Compact Ticker",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::SingleLineBottom => Self::DualLineBilingual,
            Self::DualLineBilingual => Self::MultiLineWrapped,
            Self::MultiLineWrapped => Self::CompactTicker,
            Self::CompactTicker => Self::SingleLineBottom,
        }
    }
}

/// Display priority when bilingual translation is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CaptionPriority {
    #[default]
    OriginalMain,
    TranslationMain,
    OriginalOnly,
    TranslationOnly,
}

/// Style configuration for caption typography and contrast.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptionStyle {
    pub width: f32,
    pub font_size: f32,
    pub translation_font_size: f32,
    pub text_stroke: bool,
    pub stroke_width: f32,
    pub stroke_color: Hsla,
    pub text_shadow: bool,
    pub background_opacity: f32,
    pub disappear_delay_secs: f32,
    pub text_align_center: bool,
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self {
            width: 800.0,
            font_size: 22.0,
            translation_font_size: 16.0,
            text_stroke: true,
            stroke_width: 1.0,
            stroke_color: hsla(0.0, 0.0, 0.0, 0.95),
            text_shadow: true,
            background_opacity: 0.75,
            disappear_delay_secs: 4.0,
            text_align_center: true,
        }
    }
}

impl CaptionStyle {
    /// Sanitizes and returns the disappearance delay in seconds.
    /// Clamps to `[0.5, 30.0]`. If `disappear_delay_secs` is non-positive,
    /// NaN, or non-finite, falls back to the default of `4.0` seconds.
    pub fn sanitized_disappear_delay_secs(&self) -> f32 {
        if self.disappear_delay_secs.is_finite() && self.disappear_delay_secs > 0.0 {
            self.disappear_delay_secs.clamp(0.5, 30.0)
        } else {
            4.0
        }
    }

    /// Returns a validated `std::time::Duration` for the disappearance delay.
    /// Guaranteed not to panic.
    pub fn disappear_delay(&self) -> Duration {
        Duration::from_secs_f32(self.sanitized_disappear_delay_secs())
    }
}

/// Root GPUI View entity for the Floating Caption Overlay Window.
pub struct CaptionOverlayWindow {
    pub segments: Vec<TranscriptSegment>,
    pub layout_mode: CaptionLayoutMode,
    pub priority: CaptionPriority,
    pub style: CaptionStyle,
    pub is_locked: bool,
    pub is_hovered: bool,
    pub is_fading_out: bool,
    pub toast_message: Option<String>,
    pub translation_error: Option<String>,
    pub disappear_task: Option<Task<()>>,
    pub toast_task: Option<Task<()>>,
    pub hover_task: Option<Task<()>>,
    pub _event_subscription_task: Option<Task<()>>,
}

impl CaptionOverlayWindow {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut overlay = Self {
            segments: Vec::new(),
            layout_mode: CaptionLayoutMode::DualLineBilingual,
            priority: CaptionPriority::OriginalMain,
            style: CaptionStyle::default(),
            is_locked: false,
            is_hovered: false,
            is_fading_out: false,
            toast_message: None,
            translation_error: None,
            disappear_task: None,
            toast_task: None,
            hover_task: None,
            _event_subscription_task: None,
        };

        overlay.subscribe_to_event_bus(cx);
        overlay
    }

    /// Subscribes to global backend events emitted by `AppEventBus`.
    fn subscribe_to_event_bus(&mut self, cx: &mut Context<Self>) {
        let global_bus = cx.global::<GlobalEventBus>();
        let mut rx = global_bus.0.subscribe();

        self._event_subscription_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            while let Ok(event) = rx.recv().await {
                let this = this.clone();
                let _ = this.update(cx, |view, cx| {
                    view.handle_app_event(&event, cx);
                });
            }
        }));
    }

    /// Handles incoming broadcast events from the backend.
    fn handle_app_event(&mut self, event: &AppEvent, cx: &mut Context<Self>) {
        if event.name.starts_with("recognizer-output-") {
            if let Ok(update) = event.decode::<TranscriptUpdate>() {
                self.apply_transcript_update(update, cx);
            }
        } else if event.is("caption:lock-toggle") {
            self.toggle_lock(cx);
        } else if event.is("realtime-translation-error") {
            if let Ok(error_info) = event.decode::<serde_json::Value>() {
                let msg = error_info["error"].as_str().unwrap_or("Translation error");
                self.translation_error = Some(msg.to_string());
                cx.notify();
            }
        }
    }

    /// Applies live transcription updates and resets disappearance timer.
    pub fn apply_transcript_update(&mut self, update: TranscriptUpdate, cx: &mut Context<Self>) {
        if self.translation_error.is_some() {
            self.translation_error = None;
        }

        // 1. Remove deleted IDs
        if !update.remove_ids.is_empty() {
            self.segments.retain(|s| !update.remove_ids.contains(&s.id));
        }

        // 2. Upsert segments
        for new_seg in update.upsert_segments {
            if let Some(pos) = self.segments.iter().position(|s| s.id == new_seg.id) {
                self.segments[pos] = new_seg;
            } else {
                self.segments.push(new_seg);
            }
        }

        // 3. Keep memory bounded to latest 6 segments
        if self.segments.len() > 6 {
            let overflow = self.segments.len() - 6;
            self.segments.drain(0..overflow);
        }

        self.is_fading_out = false;
        self.reset_disappear_timer(cx);
        cx.notify();
    }

    /// Resets the smooth auto-disappearance timer.
    fn reset_disappear_timer(&mut self, cx: &mut Context<Self>) {
        self.disappear_task = None;
        let delay = self.style.disappear_delay();

        self.disappear_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            tokio::time::sleep(delay).await;
            let _ = this.update(cx, |view, cx| {
                view.is_fading_out = true;
                cx.notify();
            });

            tokio::time::sleep(Duration::from_millis(300)).await;
            let _ = this.update(cx, |view, cx| {
                view.segments.clear();
                view.is_fading_out = false;
                cx.notify();
            });
        }));
    }

    /// Updates hover state and manages auto-dismiss lifecycle for the hover toolbar.
    pub fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            self.is_hovered = false;
            self.hover_task = None;
            cx.notify();
        } else {
            self.is_hovered = true;
            cx.notify();

            // Auto-dismiss toolbar after 4 seconds of stillness/inactivity as fallback safety
            self.hover_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
                tokio::time::sleep(Duration::from_secs(4)).await;
                let _ = this.update(cx, |view, cx| {
                    view.is_hovered = false;
                    cx.notify();
                });
            }));
        }
    }

    /// Refreshes the auto-dismiss timer during active mouse movement over the overlay.
    pub fn ping_hover_activity(&mut self, cx: &mut Context<Self>) {
        if self.is_hovered && !self.is_locked {
            self.hover_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
                tokio::time::sleep(Duration::from_secs(4)).await;
                let _ = this.update(cx, |view, cx| {
                    view.is_hovered = false;
                    cx.notify();
                });
            }));
        }
    }

    /// Toggles click-through mode via Win32 `WS_EX_TRANSPARENT`.
    pub fn toggle_lock(&mut self, cx: &mut Context<Self>) {
        self.is_locked = !self.is_locked;
        set_platform_click_through(CAPTION_WINDOW_TITLE, self.is_locked);
        if self.is_locked {
            self.is_hovered = false;
            self.hover_task = None;
        }

        let msg = if self.is_locked {
            "Click-through locked (Alt+C to unlock)"
        } else {
            "Click-through unlocked"
        };
        self.show_toast(msg, cx);
        cx.notify();
    }

    /// Copies the currently displayed subtitle segment to the system clipboard.
    pub fn copy_current_text(&mut self, cx: &mut Context<Self>) {
        if let Some(latest) = self.segments.last() {
            let mut text = latest.text.clone();
            if let Some(ref trans) = latest.translation {
                text.push('\n');
                text.push_str(trans);
            }
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.show_toast("Copied Subtitle", cx);
        }
    }

    /// Cycles through the 4 layout modes.
    pub fn cycle_layout_mode(&mut self, cx: &mut Context<Self>) {
        self.layout_mode = self.layout_mode.next();
        self.show_toast(self.layout_mode.label(), cx);
        cx.notify();
    }

    /// Displays a temporary floating toast message over the caption window.
    pub fn show_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.toast_message = Some(message.into());
        self.toast_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            tokio::time::sleep(Duration::from_millis(2000)).await;
            let _ = this.update(cx, |view, cx| {
                view.toast_message = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Renders high-contrast text with 4-way stroke outline and diffuse shadow.
    fn render_stroked_text(
        &self,
        content: &str,
        size_px: f32,
        weight: FontWeight,
        text_color: Hsla,
    ) -> impl IntoElement {
        let content_owned = content.to_string();
        let stroke_color = self.style.stroke_color;
        let shadow_enabled = self.style.text_shadow;
        let stroke_enabled = self.style.text_stroke;

        div()
            .relative()
            .flex()
            .items_center()
            // Ambient soft diffuse shadow
            .when(shadow_enabled, |this| this.shadow_sm())
            // 4-way outline layers for wallpaper contrast
            .when(stroke_enabled, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .text_size(px(size_px))
                        .font_weight(weight)
                        .text_color(stroke_color)
                        .mt(px(-1.0))
                        .ml(px(-1.0))
                        .child(content_owned.clone()),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .text_size(px(size_px))
                        .font_weight(weight)
                        .text_color(stroke_color)
                        .mt(px(-1.0))
                        .ml(px(1.0))
                        .child(content_owned.clone()),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .text_size(px(size_px))
                        .font_weight(weight)
                        .text_color(stroke_color)
                        .mt(px(1.0))
                        .ml(px(-1.0))
                        .child(content_owned.clone()),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .text_size(px(size_px))
                        .font_weight(weight)
                        .text_color(stroke_color)
                        .mt(px(1.0))
                        .ml(px(1.0))
                        .child(content_owned.clone()),
                )
            })
            // Foreground main text layer
            .child(
                div()
                    .relative()
                    .text_size(px(size_px))
                    .font_weight(weight)
                    .text_color(text_color)
                    .child(content_owned),
            )
    }

    /// Renders layout mode 1: Single-Line Bottom.
    fn render_single_line(&self) -> impl IntoElement {
        let latest = self.segments.last();
        let display_text = match latest {
            Some(seg) => {
                if self.priority == CaptionPriority::TranslationOnly && seg.translation.is_some() {
                    seg.translation.clone().unwrap()
                } else {
                    seg.text.clone()
                }
            }
            None => "Waiting for speech...".to_string(),
        };

        div()
            .flex()
            .justify_center()
            .items_center()
            .size_full()
            .child(self.render_stroked_text(
                &display_text,
                self.style.font_size,
                FontWeight::SEMIBOLD,
                gpui_kit::white(),
            ))
    }

    /// Renders layout mode 2: Dual-Line Bilingual.
    fn render_dual_line(&self) -> impl IntoElement {
        let latest = self.segments.last();
        let original = latest
            .map(|s| s.text.as_str())
            .unwrap_or("Waiting for speech...");
        let translation = latest.and_then(|s| s.translation.as_deref()).unwrap_or("");
        let draft_translation = latest
            .and_then(|s| s.draft_translation.as_deref())
            .unwrap_or("");

        let trans_color = hsla(160.0 / 360.0, 0.84, 0.65, 1.0); // Mint emerald
        let is_translation_main = self.priority == CaptionPriority::TranslationMain;

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .size_full()
            .when(!is_translation_main, |this| {
                this.child(self.render_stroked_text(
                    original,
                    self.style.font_size,
                    FontWeight::SEMIBOLD,
                    gpui_kit::white(),
                ))
                .when(
                    !translation.is_empty() || !draft_translation.is_empty(),
                    |this| {
                        let combined = if !draft_translation.is_empty() {
                            format!("{} {}", translation, draft_translation)
                        } else {
                            translation.to_string()
                        };
                        this.child(self.render_stroked_text(
                            &combined,
                            self.style.translation_font_size,
                            FontWeight::MEDIUM,
                            trans_color,
                        ))
                    },
                )
            })
            .when(is_translation_main, |this| {
                let trans_text = if !translation.is_empty() {
                    translation
                } else {
                    original
                };
                this.child(self.render_stroked_text(
                    trans_text,
                    self.style.font_size,
                    FontWeight::SEMIBOLD,
                    trans_color,
                ))
                .when(!translation.is_empty(), |this| {
                    this.child(self.render_stroked_text(
                        original,
                        self.style.translation_font_size,
                        FontWeight::MEDIUM,
                        hsla(0.0, 0.0, 0.9, 0.8),
                    ))
                })
            })
    }

    /// Renders layout mode 3: Multi-Line Wrapped stream.
    fn render_multi_line(&self) -> impl IntoElement {
        let count = self.segments.len();
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .size_full()
            .children(self.segments.iter().enumerate().map(|(idx, seg)| {
                let is_latest = idx == count.saturating_sub(1);
                let text_color = if is_latest {
                    gpui_kit::white()
                } else {
                    hsla(0.0, 0.0, 0.85, 0.65)
                };
                let font_size = if is_latest {
                    self.style.font_size
                } else {
                    self.style.font_size * 0.9
                };

                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.render_stroked_text(
                        &seg.text,
                        font_size,
                        if is_latest {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        },
                        text_color,
                    ))
            }))
    }

    /// Renders layout mode 4: Compact Ticker.
    fn render_compact_ticker(&self) -> impl IntoElement {
        let latest = self.segments.last();
        let display_text = latest.map(|s| s.text.as_str()).unwrap_or("Ready");
        let speaker_tag = latest
            .and_then(|s| s.speaker.as_ref())
            .map(|sp| sp.label.as_str())
            .unwrap_or("Live");

        div()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py_1()
            .rounded_full()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .border_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.15))
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded(px(4.0))
                    .bg(hsla(217.0 / 360.0, 0.91, 0.60, 0.25))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(hsla(217.0 / 360.0, 0.91, 0.60, 1.0))
                    .child(speaker_tag.to_string()),
            )
            .child(self.render_stroked_text(
                display_text,
                15.0,
                FontWeight::MEDIUM,
                gpui_kit::white(),
            ))
    }
}

impl Render for CaptionOverlayWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = hsla(0.0, 0.0, 0.05, self.style.background_opacity);
        let opacity = if self.is_fading_out { 0.0 } else { 1.0 };

        div()
            .id("caption-overlay-root")
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .rounded(px(16.0))
            .bg(bg_color)
            .border_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.12))
            .shadow_lg()
            .opacity(opacity)
            .on_hover(cx.listener(|this, hovered: &bool, _window, cx| {
                this.set_hovered(*hovered, cx);
            }))
            .on_mouse_move(cx.listener(|this, _event, _window, cx| {
                this.ping_hover_activity(cx);
            }))
            // Hover Toolbar (Drag handle, Copy, Lock, Layout mode)
            .when(self.is_hovered && !self.is_locked, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .mb_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(hsla(0.0, 0.0, 0.1, 0.65))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(hsla(0.0, 0.0, 0.7, 1.0))
                                .child("Caption Overlay"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("btn-cycle-mode")
                                        .ghost()
                                        .xsmall()
                                        .label(self.layout_mode.label())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cycle_layout_mode(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btn-copy-subtitle")
                                        .ghost()
                                        .xsmall()
                                        .label("Copy")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.copy_current_text(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btn-lock-overlay")
                                        .ghost()
                                        .xsmall()
                                        .label(if self.is_locked { "Locked" } else { "Lock" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_lock(cx);
                                        })),
                                ),
                        ),
                )
            })
            // Toast notification banner
            .when_some(self.toast_message.clone(), |this, msg| {
                this.child(
                    div()
                        .w_full()
                        .py_1()
                        .px_3()
                        .mb_2()
                        .rounded_md()
                        .bg(hsla(217.0 / 360.0, 0.91, 0.60, 0.85))
                        .text_size(px(12.0))
                        .text_color(gpui_kit::white())
                        .child(msg),
                )
            })
            // Error banner
            .when_some(self.translation_error.clone(), |this, err| {
                this.child(
                    div()
                        .w_full()
                        .py_1()
                        .px_3()
                        .mb_2()
                        .rounded_md()
                        .bg(hsla(0.0, 0.75, 0.50, 0.85))
                        .text_size(px(12.0))
                        .text_color(gpui_kit::white())
                        .child(err),
                )
            })
            // Subtitle Content matching selected layout mode
            .child(div().flex_1().flex().items_center().justify_center().child(
                match self.layout_mode {
                    CaptionLayoutMode::SingleLineBottom => {
                        self.render_single_line().into_any_element()
                    }
                    CaptionLayoutMode::DualLineBilingual => {
                        self.render_dual_line().into_any_element()
                    }
                    CaptionLayoutMode::MultiLineWrapped => {
                        self.render_multi_line().into_any_element()
                    }
                    CaptionLayoutMode::CompactTicker => {
                        self.render_compact_ticker().into_any_element()
                    }
                },
            ))
    }
}

/// Win32 Click-Through helper via `WS_EX_TRANSPARENT`.
#[cfg(windows)]
pub fn set_platform_click_through(window_title: &str, click_through: bool) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_LAYERED,
        WS_EX_TRANSPARENT,
    };
    use windows::core::HSTRING;

    unsafe {
        let title = HSTRING::from(window_title);
        if let Ok(hwnd) = FindWindowW(None, &title) {
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let transparent_mask = (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as isize;
            let new_style = if click_through {
                ex_style | transparent_mask
            } else {
                ex_style & !(WS_EX_TRANSPARENT.0 as isize)
            };
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
            true
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
pub fn set_platform_click_through(_window_title: &str, _click_through: bool) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn test_caption_style_default_delay() {
        let style = CaptionStyle::default();
        assert_eq!(style.disappear_delay_secs, 4.0);
        assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(4));
    }

    #[test]
    fn test_caption_style_delay_sanitization_edge_cases() {
        let mut style = CaptionStyle::default();

        // Sub-minimum clamped to 0.5s
        style.disappear_delay_secs = 0.1;
        assert_eq!(style.sanitized_disappear_delay_secs(), 0.5);
        assert_eq!(style.disappear_delay(), Duration::from_secs_f32(0.5));

        // Valid custom delay
        style.disappear_delay_secs = 8.5;
        assert_eq!(style.sanitized_disappear_delay_secs(), 8.5);
        assert_eq!(style.disappear_delay(), Duration::from_secs_f32(8.5));

        // Excessively large delay clamped to 30.0s
        style.disappear_delay_secs = 120.0;
        assert_eq!(style.sanitized_disappear_delay_secs(), 30.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(30));

        // Zero falls back to 4.0s
        style.disappear_delay_secs = 0.0;
        assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(4));

        // Negative values fall back to 4.0s (prevent Duration panic)
        style.disappear_delay_secs = -5.0;
        assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(4));

        // NaN falls back to 4.0s (prevent Duration panic)
        style.disappear_delay_secs = f32::NAN;
        assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(4));

        // Positive infinity falls back to 4.0s
        style.disappear_delay_secs = f32::INFINITY;
        assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(4));

        // Negative infinity falls back to 4.0s
        style.disappear_delay_secs = f32::NEG_INFINITY;
        assert_eq!(style.sanitized_disappear_delay_secs(), 4.0);
        assert_eq!(style.disappear_delay(), Duration::from_secs(4));
    }

    #[test]
    fn test_caption_layout_mode_cycle() {
        assert_eq!(
            CaptionLayoutMode::SingleLineBottom.next(),
            CaptionLayoutMode::DualLineBilingual
        );
        assert_eq!(
            CaptionLayoutMode::DualLineBilingual.next(),
            CaptionLayoutMode::MultiLineWrapped
        );
        assert_eq!(
            CaptionLayoutMode::MultiLineWrapped.next(),
            CaptionLayoutMode::CompactTicker
        );
        assert_eq!(
            CaptionLayoutMode::CompactTicker.next(),
            CaptionLayoutMode::SingleLineBottom
        );
    }
}
