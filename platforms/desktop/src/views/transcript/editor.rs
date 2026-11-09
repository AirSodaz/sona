//! Virtualized transcript segment editor list.

use gpui_kit::assets::IconName as LucideIcon;
use std::collections::HashMap;

use super::scroll_follow::{
    ScrollDetachMode, ScrollFollowState, ScrollTarget, is_near_scroll_bottom, transcript_word_count,
};
use super::workbench::TranscriptWorkbench;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::segment_row::{SegmentData, SegmentRow, SegmentSearchHighlight};

pub struct TranscriptEditor {
    pub segments: Vec<SegmentData>,
    pub editing_id: Option<String>,
    pub search_highlights: HashMap<String, Vec<SegmentSearchHighlight>>,
    pub active_search_match: Option<usize>,
    pub edit_states: HashMap<String, Entity<InputState>>,
    pub translation_editing_id: Option<String>,
    pub translation_states: HashMap<String, Entity<InputState>>,
    pub show_translations: bool,
    follow_state: ScrollFollowState,
    scroll_handle: ScrollHandle,
    pending_scroll: Option<ScrollTarget>,
    last_detach_mode: Option<ScrollDetachMode>,
}
impl TranscriptEditor {
    pub fn new(_window: &mut Window, _cx: &mut Context<TranscriptWorkbench>) -> Self {
        Self {
            segments: Vec::new(),
            editing_id: None,
            search_highlights: HashMap::new(),
            active_search_match: None,
            edit_states: HashMap::new(),
            translation_editing_id: None,
            translation_states: HashMap::new(),
            show_translations: true,
            follow_state: ScrollFollowState::default(),
            scroll_handle: ScrollHandle::new(),
            pending_scroll: None,
            last_detach_mode: None,
        }
    }

    pub fn reset_session(&mut self) {
        self.follow_state.reset_session();
        self.pending_scroll = None;
    }

    pub fn set_streaming_active(&mut self, active: bool) {
        if self.follow_state.set_streaming_active(active) && !self.segments.is_empty() {
            self.pending_scroll = Some(ScrollTarget::Bottom);
        }
    }

    pub fn follow_new_segments(&mut self) {
        if self.follow_state.is_streaming_following() {
            self.pending_scroll = Some(ScrollTarget::Bottom);
        }
    }

    pub fn update_playback(&mut self, playing: bool, seeked: bool, current_time: f64) {
        let active_segment = self.segments.iter().position(|segment| {
            current_time >= segment.start_time && current_time <= segment.end_time
        });
        let target = self.follow_state.update_playback(
            playing,
            seeked,
            self.editing_id.is_some(),
            active_segment,
        );
        if target.is_some() {
            self.pending_scroll = target;
        }
    }

    fn resume_follow(&mut self) {
        self.pending_scroll = self.follow_state.resume();
    }

    fn apply_pending_scroll(&mut self) {
        match self.pending_scroll {
            Some(ScrollTarget::Bottom) => {
                self.scroll_handle.scroll_to_bottom();
                self.pending_scroll = None;
            }
            Some(ScrollTarget::Segment(ix)) => {
                if let Some(bounds) = self.scroll_handle.bounds_for_item(ix) {
                    let viewport = self.scroll_handle.bounds();
                    let current = self.scroll_handle.offset();
                    let centered_y = current.y + viewport.center().y - bounds.center().y;
                    self.scroll_handle.set_offset(point(current.x, centered_y));
                    self.pending_scroll = None;
                } else {
                    self.scroll_handle.scroll_to_item(ix);
                }
            }
            None => {}
        }
    }

    fn ensure_edit_state(
        &mut self,
        id: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<TranscriptWorkbench>,
    ) -> Entity<InputState> {
        if let Some(state) = self.edit_states.get(id) {
            return state.clone();
        }
        let value = value.to_string();
        let state = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Transcript text");
            state.set_value(value, window, cx);
            state
        });
        self.edit_states.insert(id.to_string(), state.clone());
        state
    }

    pub fn begin_edit(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<TranscriptWorkbench>,
    ) {
        if let Some(segment) = self
            .segments
            .iter()
            .find(|segment| segment.id == id)
            .cloned()
        {
            let state = self.ensure_edit_state(id, &segment.text, window, cx);
            state.update(cx, |state, cx| state.focus(window, cx));
            self.editing_id = Some(id.to_string());
        }
    }

    pub fn save_edit(&mut self, id: &str, cx: &mut Context<TranscriptWorkbench>) {
        let Some(state) = self.edit_states.get(id) else {
            return;
        };
        let value = state.read(cx).value().to_string();
        if let Some(segment) = self.segments.iter_mut().find(|segment| segment.id == id) {
            segment.text = value;
            segment.timing = None;
        }
        self.editing_id = None;
    }

    pub fn cancel_edit(&mut self) {
        self.editing_id = None;
    }

    fn ensure_translation_state(
        &mut self,
        id: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<TranscriptWorkbench>,
    ) -> Entity<InputState> {
        if let Some(state) = self.translation_states.get(id) {
            return state.clone();
        }
        let value = value.to_string();
        let state = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Translation");
            state.set_value(value, window, cx);
            state
        });
        self.translation_states
            .insert(id.to_string(), state.clone());
        state
    }

    pub fn begin_translation_edit(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<TranscriptWorkbench>,
    ) {
        let translation = self
            .segments
            .iter()
            .find(|segment| segment.id == id)
            .and_then(|segment| segment.translation.clone());
        if let Some(translation) = translation {
            let state = self.ensure_translation_state(id, &translation, window, cx);
            state.update(cx, |state, cx| state.focus(window, cx));
            self.translation_editing_id = Some(id.to_string());
        }
    }

    pub fn save_translation(&mut self, id: &str, cx: &mut Context<TranscriptWorkbench>) {
        let Some(state) = self.translation_states.get(id) else {
            return;
        };
        let value = state.read(cx).value().trim().to_string();
        if let Some(segment) = self.segments.iter_mut().find(|segment| segment.id == id) {
            segment.translation = if value.is_empty() { None } else { Some(value) };
        }
        self.translation_editing_id = None;
    }

    pub fn delete_segment(&mut self, id: &str) {
        self.segments.retain(|segment| segment.id != id);
        self.edit_states.remove(id);
        if self.editing_id.as_deref() == Some(id) {
            self.editing_id = None;
        }
    }

    pub fn merge_with_next(&mut self, id: &str) {
        let Some(index) = self.segments.iter().position(|segment| segment.id == id) else {
            return;
        };
        if index + 1 >= self.segments.len() {
            return;
        }
        let next = self.segments.remove(index + 1);
        let current = &mut self.segments[index];
        if !current
            .text
            .chars()
            .last()
            .is_some_and(|ch| ch.is_whitespace())
        {
            current.text.push(' ');
        }
        current.text.push_str(&next.text);
        current.end_time = next.end_time;
        current.timing = None;
    }

    pub fn split_segment(&mut self, id: &str) {
        let Some(index) = self.segments.iter().position(|segment| segment.id == id) else {
            return;
        };
        let segment = self.segments[index].clone();
        let words: Vec<&str> = segment.text.split_whitespace().collect();
        if words.len() < 2 {
            return;
        }
        let midpoint = words.len() / 2;
        let left_text = words[..midpoint].join(" ");
        let right_text = words[midpoint..].join(" ");
        let split_time = segment.start_time
            + (segment.end_time - segment.start_time) * midpoint as f64 / words.len() as f64;
        self.segments[index].text = left_text;
        self.segments[index].end_time = split_time;
        self.segments[index].timing = None;
        self.segments.insert(
            index + 1,
            SegmentData {
                id: format!("{}-split", segment.id),
                start_time: split_time,
                end_time: segment.end_time,
                speaker: segment.speaker,
                is_final: segment.is_final,
                speaker_tag: segment.speaker_tag,
                speaker_attribution: segment.speaker_attribution,
                timing: None,
                text: right_text,
                translation: None,
            },
        );
    }

    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<TranscriptWorkbench>,
    ) -> impl IntoElement {
        if self.segments.is_empty() {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .size_full()
                .gap_3()
                .text_color(cx.theme().muted)
                .child(Icon::new(LucideIcon::CirclePlus).size(px(28.0)))
                .child("No transcript segments yet")
                .into_any_element();
        }

        self.apply_pending_scroll();
        if self.follow_state.is_streaming_following() {
            self.scroll_handle.scroll_to_bottom();
        }

        let word_count =
            transcript_word_count(self.segments.iter().map(|segment| segment.text.as_str()));
        let detach_mode = self.follow_state.detach_mode();
        if detach_mode.is_some() {
            self.last_detach_mode = detach_mode;
        }
        let display_mode = detach_mode.or(self.last_detach_mode);

        let current_time = cx.entity().read(cx).player.current_time;
        let content = div()
            .id("transcript-editor-scroll")
            .flex()
            .flex_col()
            .size_full()
            .track_scroll(&self.scroll_handle)
            .overflow_y_scroll()
            .vertical_scrollbar(&self.scroll_handle)
            .p_4()
            .gap_2()
            .on_scroll(cx.listener(|this, _, _, cx| {
                let offset = this.editor.scroll_handle.offset();
                let max = this.editor.scroll_handle.max_offset().y;
                let at_bottom = is_near_scroll_bottom(offset.y.as_f32(), max.as_f32());
                this.editor.follow_state.on_user_scroll(at_bottom);
                cx.notify();
            }))
            .children({
                let segments = self.segments.clone();
                let mut rows = Vec::with_capacity(segments.len());
                for seg in segments {
                    let editing = self.editing_id.as_deref() == Some(seg.id.as_str());
                    let state = if editing {
                        Some(self.ensure_edit_state(&seg.id, &seg.text, window, cx))
                    } else {
                        None
                    };
                    let translation_editing =
                        self.translation_editing_id.as_deref() == Some(seg.id.as_str());
                    let translation_state = if translation_editing {
                        seg.translation.as_deref().map(|translation| {
                            self.ensure_translation_state(&seg.id, translation, window, cx)
                        })
                    } else {
                        None
                    };
                    let search_highlights = self
                        .search_highlights
                        .get(&seg.id)
                        .cloned()
                        .unwrap_or_default();
                    rows.push(SegmentRow::render_interactive(
                        seg,
                        false,
                        editing,
                        state,
                        translation_editing,
                        translation_state,
                        self.show_translations,
                        current_time,
                        search_highlights,
                        self.active_search_match,
                        cx,
                    ));
                }
                rows
            });

        div()
            .relative()
            .size_full()
            .child(content)
            .when(word_count > 0 || detach_mode.is_some(), |this| {
                let present = detach_mode.is_some();
                let sample = gpui_kit::base::motion::Presence::new(
                    "transcript-scroll-follow-action",
                    present,
                )
                .transition(gpui_kit::base::motion::Transition::new(
                    std::time::Duration::from_millis(280),
                ))
                .sample(window, cx);
                let mode = display_mode.unwrap_or(ScrollDetachMode::Streaming);
                let icon = match mode {
                    ScrollDetachMode::Streaming => LucideIcon::ArrowDown,
                    ScrollDetachMode::Playback => LucideIcon::LocateFixed,
                };
                let tooltip = match mode {
                    ScrollDetachMode::Streaming => "Scroll to bottom and resume follow",
                    ScrollDetachMode::Playback => "Locate playing segment",
                };
                this.child(
                    div()
                        .absolute()
                        .bottom(px(12.0))
                        .right(px(16.0))
                        .flex()
                        .items_center()
                        .gap(px(if present { 6.0 } else { 0.0 }))
                        .px(px(if present { 6.0 } else { 10.0 }))
                        .py(px(4.0))
                        .rounded_full()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(if present {
                            cx.theme().primary
                        } else {
                            cx.theme().border
                        })
                        .shadow_sm()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .font_family("monospace")
                        .child(
                            div()
                                .w(px(28.0 * sample.progress))
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .overflow_hidden()
                                .opacity(sample.progress)
                                .when(sample.should_render(), |this| {
                                    this.child(
                                        Button::new("transcript-scroll-follow-resume")
                                            .ghost()
                                            .xsmall()
                                            .icon(Icon::new(icon).size(px(13.0)))
                                            .tooltip(tooltip)
                                            .accessibility_label(tooltip)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.editor.resume_follow();
                                                cx.notify();
                                            })),
                                    )
                                }),
                        )
                        .child(format!("{word_count} words")),
                )
            })
            .into_any_element()
    }
}
