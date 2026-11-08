//! Floating Voice Typing Dynamic Island widget (Gap G15).
//! Pill-shaped floating dynamic island with 5 animated waveform bars reactive to audio
//! peak levels from CPAL, 7-phase state machine transitions, context badges, and quick recall drawer.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::app::{GlobalDesktopServices, GlobalEventBus};
use crate::event_bus::AppEvent;
use crate::integrations::audio::MICROPHONE_PEAK_EVENT;
use crate::platform::event::EventEmitterPort;
use crate::platform::system::host::get_foreground_window_info;

/// 5 Waveform bar height weights matching reference React frontend.
const WAVEFORM_WEIGHTS: [f32; 5] = [0.45, 0.75, 1.0, 0.75, 0.45];
const MIN_BAR_HEIGHT: f32 = 4.0;
const MAX_BAR_HEIGHT: f32 = 18.0;
const MAX_HISTORY_ITEMS: usize = 100;

/// State machine phases for Voice Typing dictation workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VoiceTypingPhase {
    #[default]
    Listening,
    Transcribing,
    Polishing,
    Translating,
    Done,
    Error,
    Recall,
}

impl VoiceTypingPhase {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Listening | Self::Transcribing | Self::Polishing | Self::Translating
        )
    }
}

/// Target application context kinds recognized by the dynamic island.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextPreset {
    SelectionRewrite,
    Developer(String),
    Chat(String),
    Formal(String),
    General(String),
}

/// History record for quick recall drawer.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoiceTypingHistorySnippet {
    pub id: String,
    #[serde(alias = "injectedText")]
    pub injected_text: String,
    #[serde(default, alias = "timestamp")]
    pub timestamp_ms: u64,
}

/// Root GPUI View entity for the Voice Typing Dynamic Island.
pub struct VoiceTypingIsland {
    pub phase: VoiceTypingPhase,
    pub interim_text: String,
    pub translation_text: Option<String>,
    pub target_language: String,
    pub audio_peak_volume: f32, // 0.0 to 1.0
    pub context_preset: Option<ContextPreset>,
    pub has_selection: bool,
    pub error_message: Option<String>,
    pub history: Vec<VoiceTypingHistorySnippet>,
    pub is_drawer_open: bool,
    pub fade_task: Option<Task<()>>,
    pub _event_subscription_task: Option<Task<()>>,
}

impl VoiceTypingIsland {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>, initial_recall: bool) -> Self {
        let history = cx
            .global::<GlobalDesktopServices>()
            .config
            .get_setting("voiceTypingHistory")
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        let mut island = Self {
            phase: if initial_recall {
                VoiceTypingPhase::Recall
            } else {
                VoiceTypingPhase::Listening
            },
            interim_text: "Listening...".to_string(),
            translation_text: None,
            target_language: "EN".to_string(),
            audio_peak_volume: 0.0,
            context_preset: None,
            has_selection: false,
            error_message: None,
            history,
            is_drawer_open: initial_recall,
            fade_task: None,
            _event_subscription_task: None,
        };

        island.refresh_active_context();
        island.subscribe_to_event_bus(cx);
        island
    }

    /// Subscribes to global audio peaks and voice typing session events.
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

    /// Handles events from `AppEventBus`.
    fn handle_app_event(&mut self, event: &AppEvent, cx: &mut Context<Self>) {
        // 1. Microphone Peak Audio reactive updates (rectified absolute magnitude)
        if event.is(MICROPHONE_PEAK_EVENT) {
            if let Ok(peak_i16) = event.decode::<i16>() {
                self.audio_peak_volume = ((peak_i16 as f32).abs() / 32767.0).clamp(0.0, 1.0);
                cx.notify();
            }
        }
        // 2. Voice typing text & phase state updates
        else if event.is("voice-typing:text") {
            if let Ok(payload) = event.decode::<serde_json::Value>() {
                if let Some(text) = payload["text"].as_str() {
                    self.interim_text = text.to_string();
                }
                if let Some(phase_str) = payload["phase"].as_str() {
                    self.set_phase_from_str(phase_str, cx);
                }
                if let Some(trans) = payload["translation"].as_str() {
                    self.translation_text = Some(trans.to_string());
                }
                if let Some(has_selection) = payload["hasSelection"].as_bool() {
                    self.has_selection = has_selection;
                    if has_selection {
                        self.context_preset = Some(ContextPreset::SelectionRewrite);
                    }
                }
                if let Some(context_name) = payload["contextName"].as_str() {
                    self.context_preset = Some(ContextPreset::General(context_name.to_string()));
                } else if let Some(context_mode) = payload["contextMode"].as_str() {
                    self.context_preset = Some(match context_mode {
                        "developer" => ContextPreset::Developer(
                            payload["contextIcon"]
                                .as_str()
                                .unwrap_or("Code")
                                .to_string(),
                        ),
                        "chat" => ContextPreset::Chat(
                            payload["contextIcon"]
                                .as_str()
                                .unwrap_or("Chat")
                                .to_string(),
                        ),
                        "formal" => ContextPreset::Formal(
                            payload["contextIcon"]
                                .as_str()
                                .unwrap_or("Formal")
                                .to_string(),
                        ),
                        _ => ContextPreset::General(context_mode.to_string()),
                    });
                }

                // Ingest history array if provided in overlay payload
                if let Some(history_val) = payload.get("history").and_then(|h| h.as_array()) {
                    let mut items = Vec::new();
                    for h in history_val {
                        if let Ok(item) =
                            serde_json::from_value::<VoiceTypingHistorySnippet>(h.clone())
                        {
                            items.push(item);
                        } else if let Some(injected) = h
                            .get("injectedText")
                            .or_else(|| h.get("injected_text"))
                            .and_then(|s| s.as_str())
                        {
                            items.push(VoiceTypingHistorySnippet {
                                id: h
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                injected_text: injected.to_string(),
                                timestamp_ms: h
                                    .get("timestamp")
                                    .or_else(|| h.get("timestamp_ms"))
                                    .and_then(|t| t.as_u64())
                                    .unwrap_or(0),
                            });
                        }
                    }
                    if !items.is_empty() {
                        self.history = items;
                    }
                }

                // Record finalized snippet into history upon session completion
                let is_final = payload
                    .get("isFinal")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || payload.get("phase").and_then(|v| v.as_str()) == Some("done");
                if is_final {
                    let final_text = payload
                        .get("text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| self.interim_text.clone());
                    self.push_history_snippet(&final_text);
                    self.persist_history(cx);
                }

                cx.notify();
            }
        }
        // 3. Dedicated history sync event
        else if event.is("voice-typing:history") {
            if let Ok(snippets) = event.decode::<Vec<VoiceTypingHistorySnippet>>() {
                self.history = snippets;
                self.history.truncate(MAX_HISTORY_ITEMS);
                self.persist_history(cx);
                cx.notify();
            }
        }
        // 4. Quick recall toggle
        else if event.is("voice-typing:recall-toggle") {
            self.toggle_drawer(cx);
        }
    }

    /// Records a finalized dictation snippet into history (newest first, max 20, deduplicated).
    pub fn push_history_snippet(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed == "Listening..." {
            return;
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.history.insert(
            0,
            VoiceTypingHistorySnippet {
                id: format!("vt-{}", uuid::Uuid::new_v4()),
                injected_text: trimmed.to_string(),
                timestamp_ms: now_ms,
            },
        );
        if self.history.len() > MAX_HISTORY_ITEMS {
            self.history.truncate(MAX_HISTORY_ITEMS);
        }
    }

    fn persist_history(&self, cx: &Context<Self>) {
        let history = match serde_json::to_value(&self.history) {
            Ok(history) => history,
            Err(error) => {
                log::warn!("Failed to serialize voice typing history: {error}");
                return;
            }
        };
        let config = cx.global::<GlobalDesktopServices>().config.clone();
        if let Err(error) = config.set_setting("voiceTypingHistory", &history) {
            log::warn!("Failed to persist voice typing history: {error}");
        }
    }

    /// Maps string phase names to `VoiceTypingPhase`.
    fn set_phase_from_str(&mut self, phase_str: &str, cx: &mut Context<Self>) {
        self.phase = match phase_str {
            "listening" => VoiceTypingPhase::Listening,
            "transcribing" | "segment" => VoiceTypingPhase::Transcribing,
            "polishing" => VoiceTypingPhase::Polishing,
            "translating" => VoiceTypingPhase::Translating,
            "done" => {
                self.trigger_done_fade(cx);
                VoiceTypingPhase::Done
            }
            "error" => VoiceTypingPhase::Error,
            "recall" => VoiceTypingPhase::Recall,
            _ => VoiceTypingPhase::Listening,
        };
        cx.notify();
    }

    /// Triggers smooth checkmark completion fade.
    fn trigger_done_fade(&mut self, cx: &mut Context<Self>) {
        self.fade_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let _ = this.update(cx, |view, cx| {
                view.phase = VoiceTypingPhase::Listening;
                view.interim_text = "Listening...".to_string();
                view.translation_text = None;
                cx.notify();
            });
        }));
    }

    /// Queries OS foreground application to classify target context.
    pub fn refresh_active_context(&mut self) {
        if let Ok(Some(info)) = get_foreground_window_info() {
            let app_lower = info.app_name.to_lowercase();
            let title_lower = info.window_title.to_lowercase();

            if app_lower.contains("code")
                || app_lower.contains("cursor")
                || app_lower.contains("idea")
                || app_lower.contains("zed")
            {
                self.context_preset = Some(ContextPreset::Developer(info.app_name));
            } else if app_lower.contains("slack")
                || app_lower.contains("wechat")
                || app_lower.contains("discord")
                || app_lower.contains("teams")
            {
                self.context_preset = Some(ContextPreset::Chat(info.app_name));
            } else if app_lower.contains("word")
                || app_lower.contains("notion")
                || title_lower.contains("document")
            {
                self.context_preset = Some(ContextPreset::Formal(info.app_name));
            } else if !info.app_name.is_empty() {
                self.context_preset = Some(ContextPreset::General(info.app_name));
            }
        }
    }

    /// Computes the 5 reactive waveform bar heights.
    fn compute_bar_heights(&self) -> [f32; 5] {
        let is_speaking =
            self.phase == VoiceTypingPhase::Transcribing || self.audio_peak_volume > 0.05;

        match self.phase {
            VoiceTypingPhase::Error => [4.0; 5],
            VoiceTypingPhase::Polishing | VoiceTypingPhase::Translating => {
                [6.0, 12.0, 16.0, 12.0, 6.0]
            }
            _ if is_speaking => {
                let boost = (self.audio_peak_volume * 2.2).clamp(0.15, 1.0);
                let mut heights = [4.0; 5];
                for i in 0..5 {
                    heights[i] = (MIN_BAR_HEIGHT
                        + (MAX_BAR_HEIGHT - MIN_BAR_HEIGHT) * boost * WAVEFORM_WEIGHTS[i])
                        .round();
                }
                heights
            }
            _ => [4.0, 7.0, 10.0, 7.0, 4.0], // Idle breathing pattern
        }
    }

    /// Toggles quick recall history drawer.
    pub fn toggle_drawer(&mut self, cx: &mut Context<Self>) {
        self.is_drawer_open = !self.is_drawer_open;
        if self.is_drawer_open {
            self.phase = VoiceTypingPhase::Recall;
        } else {
            self.phase = VoiceTypingPhase::Listening;
        }
        cx.notify();
    }

    /// Cancels active dictation.
    pub fn cancel_session(&mut self, cx: &mut Context<Self>) {
        let global_bus = cx.global::<GlobalEventBus>();
        let _ = global_bus
            .0
            .emit("voice-typing:cancel", serde_json::json!({}));
        self.phase = VoiceTypingPhase::Listening;
        self.interim_text = "Listening...".to_string();
        self.translation_text = None;
        cx.notify();
    }

    /// Re-injects a past snippet into the active foreground window.
    pub fn reinject_history_item(&mut self, text: &str, cx: &mut Context<Self>) {
        let global_bus = cx.global::<GlobalEventBus>();
        let _ = global_bus
            .0
            .emit("voice-typing:reinject", serde_json::json!({ "text": text }));
        self.is_drawer_open = false;
        self.phase = VoiceTypingPhase::Listening;
        cx.notify();
    }

    /// Renders the 5 animated waveform bars with phase-reactive color gradients.
    fn render_waveform(&self) -> impl IntoElement {
        let heights = self.compute_bar_heights();

        let bar_color = match self.phase {
            VoiceTypingPhase::Error => hsla(0.0, 0.85, 0.65, 1.0),
            VoiceTypingPhase::Polishing => hsla(271.0 / 360.0, 0.81, 0.56, 1.0), // Purple
            VoiceTypingPhase::Translating => hsla(217.0 / 360.0, 0.91, 0.60, 1.0), // Blue
            VoiceTypingPhase::Transcribing => hsla(142.0 / 360.0, 0.71, 0.45, 1.0), // Emerald
            _ => hsla(142.0 / 360.0, 0.71, 0.50, 1.0),                           // Green
        };

        div()
            .flex()
            .items_center()
            .gap(px(2.5))
            .h(px(18.0))
            .children(
                heights
                    .iter()
                    .map(|&h| div().w(px(3.0)).h(px(h)).rounded_full().bg(bar_color)),
            )
    }

    /// Renders context badge pill (e.g. `[💻 VS Code]`, `[💬 Chat]`).
    fn render_context_badge(&self) -> Option<impl IntoElement> {
        let preset = self.context_preset.as_ref()?;

        // The React overlay intentionally hides the default general mode. It
        // still carries the mode internally for prompt context, but only
        // explicit developer/chat/formal or selection states get a badge.
        if matches!(preset, ContextPreset::General(_)) {
            return None;
        }

        let (label, bg_hsla, text_hsla) = match preset {
            ContextPreset::SelectionRewrite => (
                "Selection Rewrite".to_string(),
                hsla(217.0 / 360.0, 0.91, 0.60, 0.15),
                hsla(217.0 / 360.0, 0.91, 0.60, 1.0),
            ),
            ContextPreset::Developer(app) => (
                format!("💻 {}", app),
                hsla(271.0 / 360.0, 0.81, 0.56, 0.15),
                hsla(271.0 / 360.0, 0.81, 0.65, 1.0),
            ),
            ContextPreset::Chat(app) => (
                format!("💬 {}", app),
                hsla(160.0 / 360.0, 0.84, 0.39, 0.15),
                hsla(160.0 / 360.0, 0.84, 0.45, 1.0),
            ),
            ContextPreset::Formal(app) => (
                format!("📄 {}", app),
                hsla(38.0 / 360.0, 0.92, 0.50, 0.15),
                hsla(38.0 / 360.0, 0.92, 0.55, 1.0),
            ),
            ContextPreset::General(_) => unreachable!("general context is hidden above"),
        };

        Some(
            div()
                .px_2()
                .py_0p5()
                .rounded(px(4.0))
                .bg(bg_hsla)
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text_hsla)
                .child(label),
        )
    }

    /// Renders quick recall expandable history drawer.
    fn render_history_drawer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_items = !self.history.is_empty();

        div()
            .id("voice-typing-recall-drawer")
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .p_3()
            .rounded(px(16.0))
            .bg(hsla(0.0, 0.0, 0.08, 0.95))
            .border_1()
            .border_color(cx.theme().border)
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(hsla(217.0 / 360.0, 0.91, 0.60, 1.0))
                            .child("⏱")
                            .child("Quick Recall (Keys 1-5)"),
                    )
                    .child(
                        Button::new("btn-close-recall")
                            .ghost()
                            .xsmall()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_drawer(cx);
                            })),
                    ),
            )
            .child(if has_items {
                div()
                    .id("voice-typing-recall-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.history.iter().take(5).enumerate().map(|(idx, item)| {
                        let text_clone = item.injected_text.clone();
                        div()
                            .id(ElementId::from(format!("quick-recall-item-{}", idx)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_1p5()
                            .rounded_md()
                            .bg(hsla(0.0, 0.0, 0.12, 0.8))
                            .cursor_pointer()
                            .on_click(cx.listener({
                                let text_clone = text_clone.clone();
                                move |this, _, _, cx| {
                                    this.reinject_history_item(&text_clone, cx);
                                }
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded(px(3.0))
                                            .bg(hsla(0.0, 0.0, 0.2, 1.0))
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui_kit::white())
                                            .child(format!("{}", idx + 1)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(gpui_kit::white())
                                            .overflow_hidden()
                                            .child(item.injected_text.clone()),
                                    ),
                            )
                            .child(
                                Button::new(ElementId::from(format!("btn-copy-recall-{}", idx)))
                                    .ghost()
                                    .xsmall()
                                    .label("📋")
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            text_clone.clone(),
                                        ));
                                    })),
                            )
                    }))
                    .into_any_element()
            } else {
                div()
                    .id("voice-typing-recall-empty")
                    .w_full()
                    .py(px(12.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted)
                    .child("No voice typing history")
                    .into_any_element()
            })
    }
}

impl Render for VoiceTypingIsland {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_drawer_open {
            return self.render_history_drawer(cx).into_any_element();
        }

        let pill_bg = hsla(0.0, 0.0, 0.08, 0.95);

        let border_color = match self.phase {
            VoiceTypingPhase::Polishing => hsla(271.0 / 360.0, 0.81, 0.56, 0.6),
            VoiceTypingPhase::Translating => hsla(217.0 / 360.0, 0.91, 0.60, 0.6),
            VoiceTypingPhase::Error => hsla(0.0, 0.75, 0.50, 0.6),
            _ => cx.theme().border,
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .rounded(px(24.0))
            .bg(pill_bg)
            .border_1()
            .border_color(border_color)
            .shadow_lg()
            // 5 Animated Waveform Bars
            .child(self.render_waveform())
            .when(self.has_selection, |this| {
                this.child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded(px(3.0))
                        .bg(hsla(271.0 / 360.0, 0.81, 0.56, 0.2))
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(hsla(271.0 / 360.0, 0.81, 0.70, 1.0))
                        .child("Selection Rewrite"),
                )
            })
            // Context badge (if active)
            .when_some(self.render_context_badge(), |this, badge| this.child(badge))
            // Phase-specific indicators
            .when(self.phase == VoiceTypingPhase::Polishing, |this| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(hsla(271.0 / 360.0, 0.81, 0.65, 1.0))
                        .child("✨"),
                )
            })
            .when(self.phase == VoiceTypingPhase::Translating, |this| {
                this.child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(hsla(217.0 / 360.0, 0.91, 0.60, 1.0))
                        .child("Translating…"),
                )
            })
            .when(self.phase == VoiceTypingPhase::Done, |this| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(hsla(142.0 / 360.0, 0.71, 0.45, 1.0))
                        .child("✓"),
                )
            })
            // Main text stream (Original + Translation)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(gpui_kit::white())
                            .child(self.interim_text.clone()),
                    )
                    .when_some(self.translation_text.clone(), |this, trans| {
                        this.child(
                            div()
                                .text_size(px(11.5))
                                .text_color(hsla(217.0 / 360.0, 0.91, 0.60, 1.0))
                                .child(trans),
                        )
                    }),
            )
            // Actions: cancel only. Quick recall is opened by its global
            // shortcut and has its own focused drawer state in React.
            .child(
                div().flex().items_center().gap_1().child(
                    Button::new("btn-cancel-voice-typing")
                        .ghost()
                        .xsmall()
                        .label("✕")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cancel_session(cx);
                        })),
                ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn test_audio_peak_normalization_symmetry_and_clamping() {
        // Zero peak
        let zero_norm = (0.0_f32).abs() / 32767.0;
        assert_eq!(zero_norm.clamp(0.0, 1.0), 0.0);

        // Positive max peak
        let pos_max_norm = (32767.0_f32).abs() / 32767.0;
        assert_eq!(pos_max_norm.clamp(0.0, 1.0), 1.0);

        // Negative max peak (-32768) must not panic, must not clamp to 0.0
        let neg_min_norm = (-32768.0_f32).abs() / 32767.0;
        assert_eq!(neg_min_norm.clamp(0.0, 1.0), 1.0);

        // Negative mid peak (-16384) must be positive ~0.5, NOT 0.0
        let neg_mid_norm = ((-16384.0_f32).abs() / 32767.0).clamp(0.0, 1.0);
        let pos_mid_norm = ((16384.0_f32).abs() / 32767.0).clamp(0.0, 1.0);
        assert!((neg_mid_norm - pos_mid_norm).abs() < 1e-4);
        assert!((neg_mid_norm - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_history_snippet_serde_compatibility() {
        // Rust snake_case JSON
        let rust_json =
            r#"{"id":"hist-1","injected_text":"Testing Rust dictation","timestamp_ms":12345}"#;
        let snippet: VoiceTypingHistorySnippet = serde_json::from_str(rust_json).unwrap();
        assert_eq!(snippet.id, "hist-1");
        assert_eq!(snippet.injected_text, "Testing Rust dictation");
        assert_eq!(snippet.timestamp_ms, 12345);

        // TypeScript camelCase JSON matching React frontend VoiceTypingHistoryItem
        let ts_json = r#"{"id":"vt_123","injectedText":"Testing React dictation","timestamp":67890,"rawText":"raw"}"#;
        let snippet_ts: VoiceTypingHistorySnippet = serde_json::from_str(ts_json).unwrap();
        assert_eq!(snippet_ts.id, "vt_123");
        assert_eq!(snippet_ts.injected_text, "Testing React dictation");
        assert_eq!(snippet_ts.timestamp_ms, 67890);
    }

    #[test]
    fn test_push_history_snippet_preserves_repeats_and_caps_at_react_limit() {
        let mut island = VoiceTypingIsland {
            phase: VoiceTypingPhase::Listening,
            interim_text: "Listening...".to_string(),
            translation_text: None,
            target_language: "EN".to_string(),
            audio_peak_volume: 0.0,
            context_preset: None,
            has_selection: false,
            error_message: None,
            history: Vec::new(),
            is_drawer_open: false,
            fade_task: None,
            _event_subscription_task: None,
        };

        // Initially empty
        assert!(island.history.is_empty());

        // Empty text or placeholder ignored
        island.push_history_snippet("");
        island.push_history_snippet("   ");
        island.push_history_snippet("Listening...");
        assert!(island.history.is_empty());

        // Push first valid snippet
        island.push_history_snippet("First transcript");
        assert_eq!(island.history.len(), 1);
        assert_eq!(island.history[0].injected_text, "First transcript");

        // Repeated dictations remain distinct history entries, like the React store.
        island.push_history_snippet("First transcript");
        assert_eq!(island.history.len(), 2);

        // Push second valid snippet (prepended)
        island.push_history_snippet("Second transcript");
        assert_eq!(island.history.len(), 3);
        assert_eq!(island.history[0].injected_text, "Second transcript");
        assert_eq!(island.history[1].injected_text, "First transcript");

        // Verify the React store's 100-entry cap.
        for i in 3..=110 {
            island.push_history_snippet(&format!("Transcript {}", i));
        }
        assert_eq!(island.history.len(), 100);
        assert_eq!(island.history[0].injected_text, "Transcript 110");
    }

    #[test]
    fn test_waveform_bar_heights_phases() {
        let mut island = VoiceTypingIsland {
            phase: VoiceTypingPhase::Listening,
            interim_text: "Listening...".to_string(),
            translation_text: None,
            target_language: "EN".to_string(),
            audio_peak_volume: 0.0,
            context_preset: None,
            has_selection: false,
            error_message: None,
            history: Vec::new(),
            is_drawer_open: false,
            fade_task: None,
            _event_subscription_task: None,
        };

        // Idle listening breathing pattern
        assert_eq!(island.compute_bar_heights(), [4.0, 7.0, 10.0, 7.0, 4.0]);

        // Error phase
        island.phase = VoiceTypingPhase::Error;
        assert_eq!(island.compute_bar_heights(), [4.0; 5]);

        // Polishing / Translating phase
        island.phase = VoiceTypingPhase::Polishing;
        assert_eq!(island.compute_bar_heights(), [6.0, 12.0, 16.0, 12.0, 6.0]);

        island.phase = VoiceTypingPhase::Translating;
        assert_eq!(island.compute_bar_heights(), [6.0, 12.0, 16.0, 12.0, 6.0]);

        // Transcribing with reactive volume
        island.phase = VoiceTypingPhase::Transcribing;
        island.audio_peak_volume = 1.0;
        let heights = island.compute_bar_heights();
        assert!(
            heights[2] >= 17.0,
            "Center bar should reach max height: {}",
            heights[2]
        );
        assert!(
            heights[0] < heights[2],
            "Outer bars should be weighted lower"
        );
    }
}
