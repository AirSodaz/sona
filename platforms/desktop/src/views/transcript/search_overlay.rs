//! In-editor floating search widget with match counter and navigation.

use super::workbench::TranscriptWorkbench;
use crate::theme::tokens::RADIUS_LG;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptSearchMatch {
    pub segment_id: String,
    pub range: Range<usize>,
}

pub fn find_transcript_matches(
    segments: &[super::segment_row::SegmentData],
    query: &str,
) -> Vec<TranscriptSearchMatch> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    segments
        .iter()
        .flat_map(|segment| {
            find_occurrences(&segment.text, query)
                .into_iter()
                .map(|range| TranscriptSearchMatch {
                    segment_id: segment.id.clone(),
                    range,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn find_occurrences(text: &str, query: &str) -> Vec<Range<usize>> {
    let lower_query = query.to_lowercase();
    if lower_query.is_empty() {
        return Vec::new();
    }

    let mut lowered_text = String::new();
    let mut source_ranges = Vec::new();
    for (source_start, character) in text.char_indices() {
        let source_range = source_start..source_start + character.len_utf8();
        for lowered in character.to_lowercase() {
            let lowered_start = lowered_text.len();
            lowered_text.push(lowered);
            source_ranges.push((lowered_start..lowered_text.len(), source_range.clone()));
        }
    }

    lowered_text
        .char_indices()
        .filter_map(|(lowered_start, _)| {
            if !lowered_text[lowered_start..].starts_with(&lower_query) {
                return None;
            }
            let lowered_end = lowered_start + lower_query.len();
            let source_start = source_ranges
                .binary_search_by_key(&lowered_start, |(range, _)| range.start)
                .ok()
                .map(|index| source_ranges[index].1.start)?;
            let source_end = source_ranges
                .binary_search_by_key(&lowered_end, |(range, _)| range.end)
                .ok()
                .map(|index| source_ranges[index].1.end)?;
            Some(source_start..source_end)
        })
        .collect()
}

pub struct SearchOverlay {
    pub query_state: Entity<InputState>,
    pub query: String,
    pub match_count: usize,
    pub current_match: usize,
    pub is_visible: bool,
    pub matches: Vec<TranscriptSearchMatch>,
    pub matches_dirty: bool,
}

impl SearchOverlay {
    pub fn new(window: &mut Window, cx: &mut Context<TranscriptWorkbench>) -> Self {
        Self {
            query_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Find in transcript...")),
            query: String::new(),
            match_count: 0,
            current_match: 0,
            is_visible: false,
            matches: Vec::new(),
            matches_dirty: true,
        }
    }

    pub fn invalidate_matches(&mut self) {
        self.matches_dirty = true;
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<TranscriptWorkbench>) {
        self.is_visible = true;
        self.query_state
            .update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn render(&mut self, cx: &mut Context<TranscriptWorkbench>) -> impl IntoElement {
        self.query = self.query_state.read(cx).value().to_string();
        div().when(self.is_visible, |this| {
            let count_text = if self.match_count > 0 {
                format!("{}/{}", self.current_match + 1, self.match_count)
            } else if !self.query.is_empty() {
                "0/0".to_string()
            } else {
                String::new()
            };
            this.child(
                div()
                    .id("search-ui-bar")
                    .flex()
                    .items_center()
                    .min_w(px(320.0))
                    .p(px(4.0))
                    .rounded(RADIUS_LG)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_xl()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.0))
                            .child(Input::new(&self.query_state).small()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .pr(px(4.0))
                            .when(!count_text.is_empty(), |actions| {
                                actions.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .px(px(8.0))
                                        .whitespace_nowrap()
                                        .child(count_text),
                                )
                            })
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(16.0))
                                    .bg(cx.theme().border)
                                    .mx(px(4.0)),
                            )
                            .child(
                                Button::new("btn-search-prev")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::ChevronUp)
                                    .tooltip("Previous match (Shift+Enter)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.search.match_count > 0 {
                                            let count = this.search.match_count;
                                            this.search.current_match =
                                                (this.search.current_match + count - 1) % count;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("btn-search-next")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::ChevronDown)
                                    .tooltip("Next match (Enter)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.search.match_count > 0 {
                                            this.search.current_match = (this.search.current_match
                                                + 1)
                                                % this.search.match_count;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("btn-search-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::X)
                                    .tooltip("Close (Esc)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.search.is_visible = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
        })
    }
}

#[cfg(test)]
mod search_tests {
    use super::{find_occurrences, find_transcript_matches};
    use crate::views::transcript::segment_row::SegmentData;

    #[test]
    fn finds_case_insensitive_overlapping_occurrences() {
        assert_eq!(find_occurrences("Banana", "ANA"), vec![1..4, 3..6]);
    }

    #[test]
    fn maps_unicode_matches_to_utf8_byte_ranges() {
        assert_eq!(find_occurrences("A😀中a", "😀中"), vec![1..8]);
    }

    #[test]
    fn ignores_whitespace_only_queries() {
        assert!(find_occurrences("transcript", "  ").is_empty());
    }

    #[test]
    fn counts_every_match_in_transcript_order() {
        let segments = vec![
            SegmentData {
                id: "first".to_string(),
                start_time: 0.0,
                end_time: 1.0,
                speaker: String::new(),
                is_final: true,
                speaker_tag: None,
                speaker_attribution: None,
                timing: None,
                text: "one one".to_string(),
                translation: None,
            },
            SegmentData {
                id: "second".to_string(),
                start_time: 1.0,
                end_time: 2.0,
                speaker: String::new(),
                is_final: true,
                speaker_tag: None,
                speaker_attribution: None,
                timing: None,
                text: "ONE".to_string(),
                translation: None,
            },
        ];
        let matches = find_transcript_matches(&segments, "one");

        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].segment_id, "first");
        assert_eq!(matches[0].range, 0..3);
        assert_eq!(matches[1].range, 4..7);
        assert_eq!(matches[2].segment_id, "second");
        assert_eq!(matches[2].range, 0..3);
    }
}
