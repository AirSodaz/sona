//! Virtualized transcript segment editor list.

use gpui_kit::assets::IconName as LucideIcon;
use std::collections::HashMap;

use super::super::shell::MainShell;
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::*;

use super::segment_row::{SegmentData, SegmentRow};

pub struct TranscriptEditor {
    pub segments: Vec<SegmentData>,
    pub editing_id: Option<String>,
    pub search_match_id: Option<String>,
    pub edit_states: HashMap<String, Entity<InputState>>,
    pub translation_editing_id: Option<String>,
    pub translation_states: HashMap<String, Entity<InputState>>,
}

impl TranscriptEditor {
    pub fn new(_window: &mut Window, _cx: &mut Context<MainShell>) -> Self {
        Self {
            segments: Vec::new(),
            editing_id: None,
            search_match_id: None,
            edit_states: HashMap::new(),
            translation_editing_id: None,
            translation_states: HashMap::new(),
        }
    }

    fn ensure_edit_state(
        &mut self,
        id: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<MainShell>,
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

    pub fn begin_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<MainShell>) {
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

    pub fn save_edit(&mut self, id: &str, cx: &mut Context<MainShell>) {
        let Some(state) = self.edit_states.get(id) else {
            return;
        };
        let value = state.read(cx).value().to_string();
        if let Some(segment) = self.segments.iter_mut().find(|segment| segment.id == id) {
            segment.text = value;
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
        cx: &mut Context<MainShell>,
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
        cx: &mut Context<MainShell>,
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

    pub fn save_translation(&mut self, id: &str, cx: &mut Context<MainShell>) {
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
        self.segments.insert(
            index + 1,
            SegmentData {
                id: format!("{}-split", segment.id),
                start_time: split_time,
                end_time: segment.end_time,
                speaker: segment.speaker,
                text: right_text,
                translation: None,
            },
        );
    }

    pub fn render(&mut self, window: &mut Window, cx: &mut Context<MainShell>) -> impl IntoElement {
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

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scrollbar()
            .p_4()
            .gap_2()
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
                    let is_match = self.search_match_id.as_deref() == Some(seg.id.as_str());
                    rows.push(SegmentRow::render_interactive(
                        seg,
                        is_match,
                        editing,
                        state,
                        translation_editing,
                        translation_state,
                        cx,
                    ));
                }
                rows
            })
            .into_any_element()
    }
}
