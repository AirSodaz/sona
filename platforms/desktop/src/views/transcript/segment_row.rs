//! Individual transcript segment row with speaker badge, timestamp seek, and text.

use super::workbench::TranscriptWorkbench;
use crate::theme::tokens::{
    COLOR_TEXT_PRIMARY_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_FULL, RADIUS_SM,
};
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, *};
use sona_core::transcription::speaker::SpeakerProfile;
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerTag, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingUnit,
};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct SegmentData {
    pub id: String,
    pub start_time: f64,
    pub end_time: f64,
    pub speaker: String,
    pub is_final: bool,
    pub speaker_tag: Option<SpeakerTag>,
    pub speaker_attribution: Option<SpeakerAttribution>,
    pub timing: Option<TranscriptTiming>,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentSearchHighlight {
    pub range: Range<usize>,
    pub global_index: usize,
}

pub struct SegmentRow;

impl SegmentRow {
    fn aligned_units(data: &SegmentData) -> Option<&[TranscriptTimingUnit]> {
        let timing = data.timing.as_ref()?;
        if timing.level != TranscriptTimingLevel::Token
            || timing.units.is_empty()
            || timing
                .units
                .iter()
                .map(|unit| unit.text.as_str())
                .collect::<Vec<_>>()
                .concat()
                != data.text
        {
            return None;
        }
        Some(&timing.units)
    }

    fn active_unit_index(units: &[TranscriptTimingUnit], current_time: f64) -> Option<usize> {
        units.iter().enumerate().position(|(index, unit)| {
            current_time >= unit.start && (current_time < unit.end || index + 1 == units.len())
        })
    }

    fn format_timestamp(seconds: f64) -> String {
        let mins = (seconds / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        let millis = ((seconds.fract()) * 10.0).floor() as u64;
        format!("{:02}:{:02}.{}", mins, secs, millis)
    }

    pub fn render(data: &SegmentData, is_active: bool, cx: &App) -> impl IntoElement {
        let timestamp_str = Self::format_timestamp(data.start_time);

        div()
            .id(ElementId::from(format!("seg-row-{}", data.id)))
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(900.0))
            .mx_auto()
            .py_2()
            .px_3()
            .rounded(RADIUS_SM)
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .when(!data.speaker.trim().is_empty(), |this| {
                this.child(
                    div().pl(px(76.0)).mb(px(4.0)).child(
                        div()
                            .px(px(10.0))
                            .py(px(2.0))
                            .rounded(RADIUS_FULL)
                            .bg(cx.theme().secondary)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                            .child(data.speaker.clone()),
                    ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.0))
                    .w_full()
                    // Gutter: Timestamp button
                    .child(
                        div()
                            .w(px(60.0))
                            .min_w(px(60.0))
                            .flex()
                            .justify_end()
                            .child(
                                Button::new(format!("btn-seek-{}", data.id))
                                    .ghost()
                                    .xsmall()
                                    .label(timestamp_str),
                            ),
                    )
                    // Content: text + translation
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .line_height(relative(1.8))
                                    .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                    .child(data.text.clone()),
                            )
                            .when_some(data.translation.clone(), |content, trans| {
                                content.child(
                                    div()
                                        .mt(px(4.0))
                                        .text_size(px(14.5))
                                        .line_height(relative(1.5))
                                        .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                        .child(trans),
                                )
                            }),
                    ),
            )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_interactive(
        data: SegmentData,
        is_active: bool,
        editing: bool,
        edit_state: Option<Entity<InputState>>,
        translation_editing: bool,
        translation_state: Option<Entity<InputState>>,
        show_translations: bool,
        current_time: f64,
        search_highlights: Vec<SegmentSearchHighlight>,
        active_search_match: Option<usize>,
        cx: &mut Context<TranscriptWorkbench>,
    ) -> impl IntoElement + use<> {
        let id = data.id.clone();
        let timestamp = Self::format_timestamp(data.start_time);
        let is_speaker_menu_open =
            cx.entity().read(cx).speaker_menu_open_for.as_deref() == Some(id.as_str());
        div()
            .id(ElementId::from(format!("seg-row-{}", data.id)))
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(900.0))
            .mx_auto()
            .py_2()
            .px_3()
            .rounded(RADIUS_SM)
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .when(!data.speaker.trim().is_empty(), |this| {
                this.child(
                    div().pl(px(76.0)).mb(px(4.0)).child(
                        Popover::new(format!("speaker-popover-{}", id))
                            .trigger(
                                Button::new(format!("speaker-badge-{}", id))
                                    .ghost()
                                    .xsmall()
                                    .px(px(10.0))
                                    .py(px(2.0))
                                    .rounded(RADIUS_FULL)
                                    .bg(cx.theme().secondary)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                    .label(data.speaker.clone())
                                    .accessibility_label(format!("Change speaker {}", data.speaker))
                                    .disabled(!data.is_final || data.speaker_tag.is_none()),
                            )
                            .open(is_speaker_menu_open)
                            .on_open_change({
                                let workbench = cx.entity().downgrade();
                                let segment_id = id.clone();
                                move |open, _, app| {
                                    workbench
                                        .update(app, |this, cx| {
                                            if *open {
                                                this.speaker_menu_open_for =
                                                    Some(segment_id.clone());
                                                this.speaker_menu_show_all = false;
                                                let services = cx
                                                    .global::<crate::app::GlobalDesktopServices>()
                                                    .0
                                                    .clone();
                                                this.speaker_correction_profiles = services
                                                    .config
                                                    .get_setting("speakerProfiles")
                                                    .ok()
                                                    .flatten()
                                                    .and_then(|value| value.as_array().cloned())
                                                    .unwrap_or_default()
                                                    .into_iter()
                                                    .filter_map(|value| {
                                                        serde_json::from_value(value).ok()
                                                    })
                                                    .collect();
                                            } else if this.speaker_menu_open_for.as_deref()
                                                == Some(segment_id.as_str())
                                            {
                                                this.speaker_menu_open_for = None;
                                                this.speaker_menu_show_all = false;
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                }
                            })
                            .appearance(false)
                            .child(render_speaker_correction_menu(&data, cx)),
                    ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.0))
                    .w_full()
                    // Gutter (60px): Timestamp button
                    .child(
                        div()
                            .w(px(60.0))
                            .min_w(px(60.0))
                            .flex()
                            .justify_end()
                            .child(
                                Button::new(format!("btn-seek-{}", data.id))
                                    .ghost()
                                    .xsmall()
                                    .label(timestamp)
                                    .on_click(cx.listener({
                                        let time = data.start_time;
                                        move |this, _, window, cx| {
                                            this.player.seek_to(time);
                                            this.player.sync_seek_slider(window, cx);
                                            this.editor.update_playback(
                                                this.player.is_playing,
                                                true,
                                                time,
                                            );
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    // Content (1fr): text and translation
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(if editing {
                                edit_state
                                    .as_ref()
                                    .map(|state| {
                                        Input::new(state).focus_ring(true).into_any_element()
                                    })
                                    .unwrap_or_else(|| {
                                        div().child(data.text.clone()).into_any_element()
                                    })
                            } else {
                                let segment_id = id.clone();
                                if let Some(units) = Self::aligned_units(&data) {
                                    let active_unit = Self::active_unit_index(units, current_time);
                                    let token_buttons = units
                                        .iter()
                                        .enumerate()
                                        .scan(0usize, |token_start, (index, unit)| {
                                            let range_start = *token_start;
                                            *token_start += unit.text.len();
                                            Some((index, unit, range_start, *token_start))
                                        })
                                        .map(|(index, unit, token_start, token_end)| {
                                            let time = unit.start;
                                            let active = active_unit == Some(index);
                                            let matching_search = search_highlights
                                                .iter()
                                                .find(|matched| Some(matched.global_index) == active_search_match && {
                                                    matched.range.start < token_end
                                                        && matched.range.end > token_start
                                                })
                                                .or_else(|| search_highlights.iter().find(|matched| {
                                                    matched.range.start < token_end
                                                        && matched.range.end > token_start
                                                }))
                                                .cloned();
                                            Button::new(format!("transcript-token-{}-{index}", id))
                                                .ghost()
                                                .px_0()
                                                .py_0()
                                                .rounded_none()
                                                .when(active, |button| {
                                                    button.bg(
                                                        crate::theme::tokens::COLOR_TRANSCRIPT_HIGHLIGHT
                                                            .opacity(0.15),
                                                    )
                                                })
                                                .when(matching_search.is_some(), |button| {
                                                    let matched = matching_search.as_ref().unwrap();
                                                    let alpha = if Some(matched.global_index)
                                                        == active_search_match
                                                    {
                                                        0.45
                                                    } else {
                                                        0.25
                                                    };
                                                    button.bg(
                                                        crate::theme::tokens::COLOR_TRANSCRIPT_HIGHLIGHT
                                                            .opacity(alpha),
                                                    )
                                                })
                                                .label(unit.text.clone())
                                                .tooltip(Self::format_timestamp(unit.start))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.player.seek_to(time);
                                                        this.player.sync_seek_slider(window, cx);
                                                        this.editor.update_playback(
                                                            this.player.is_playing,
                                                            true,
                                                            time,
                                                        );
                                                        if let Some(matched) = &matching_search {
                                                            this.search.current_match =
                                                                matched.global_index;
                                                        }
                                                        cx.notify();
                                                    },
                                                ))
                                        })
                                        .collect::<Vec<_>>();
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .text_size(px(18.0))
                                        .line_height(relative(1.8))
                                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                        .children(token_buttons)
                                        .into_any_element()
                                } else {
                                    let highlights = search_highlights
                                        .iter()
                                        .map(|matched| {
                                            (
                                                matched.range.clone(),
                                                HighlightStyle {
                                                    background_color: Some(
                                                        crate::theme::tokens::COLOR_TRANSCRIPT_HIGHLIGHT
                                                                .opacity(if Some(matched.global_index) == active_search_match { 0.45 } else { 0.25 }),
                                                    ),
                                                    ..Default::default()
                                                },
                                            )
                                        })
                                        .collect::<Vec<_>>();
                                    let text = if highlights.is_empty() {
                                        StyledText::new(data.text.clone())
                                    } else {
                                        StyledText::new(data.text.clone()).with_highlights(highlights)
                                    };
                                    div()
                                        .text_size(px(18.0))
                                        .line_height(relative(1.8))
                                        .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                        .cursor_pointer()
                                        .child(text)
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener({
                                                let time = data.start_time;
                                                let segment_id = segment_id.clone();
                                                move |this, event: &MouseUpEvent, window, cx| {
                                                    this.player.seek_to(time);
                                                    this.player.sync_seek_slider(window, cx);
                                                    this.editor.update_playback(
                                                        this.player.is_playing,
                                                        true,
                                                        time,
                                                    );
                                                    if event.click_count == 2 {
                                                        this.editor.begin_edit(
                                                            &segment_id,
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                        .into_any_element()
                                }
                            })
                            .when(show_translations, |this| {
                                this.when_some(data.translation.clone(), |content, translation| {
                                    if translation_editing {
                                        if let Some(state) = translation_state.clone() {
                                            content.child(
                                                div()
                                                    .mt(px(4.0))
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(Input::new(&state).focus_ring(true))
                                                    .child(
                                                        Button::new(format!(
                                                            "btn-save-translation-{}",
                                                            id
                                                        ))
                                                        .primary()
                                                        .xsmall()
                                                        .label("Save")
                                                        .on_click(cx.listener({
                                                            let id = id.clone();
                                                            move |this, _, _, cx| {
                                                                this.editor
                                                                    .save_translation(&id, cx);
                                                                this.persist_editor_change(cx);
                                                                cx.notify();
                                                            }
                                                        })),
                                                    ),
                                            )
                                        } else {
                                            content.child(div().mt(px(4.0)).child(translation))
                                        }
                                    } else {
                                        content.child(
                                            div()
                                                .mt(px(4.0))
                                                .text_size(px(14.5))
                                                .line_height(relative(1.5))
                                                .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                                .child(translation),
                                        )
                                    }
                                })
                            }),
                    )
                    // Actions: buttons row with icons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(!editing, |row| {
                                row.child(
                                    Button::new(format!("btn-edit-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::Pencil)
                                        .tooltip("Edit")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, window, cx| {
                                                this.begin_edit(&id, window, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(format!("btn-merge-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::GitMerge)
                                        .tooltip("Merge with next")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.merge_with_next(&id, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(format!("btn-split-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::Split)
                                        .tooltip("Split")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.split_segment(&id, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .when(show_translations && data.translation.is_some(), |actions| {
                                    actions.child(
                                        Button::new(format!("btn-edit-translation-{}", id))
                                            .ghost()
                                            .xsmall()
                                            .icon(LucideIcon::Languages)
                                            .tooltip("Edit translation")
                                            .on_click(cx.listener({
                                                let id = id.clone();
                                                move |this, _, window, cx| {
                                                    this.editor
                                                        .begin_translation_edit(&id, window, cx);
                                                    cx.notify();
                                                }
                                            })),
                                    )
                                })
                                .child(
                                    Button::new(format!("btn-delete-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::Trash)
                                        .tooltip("Delete")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.delete_segment(&id, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                            })
                            .when(editing, |row| {
                                row.child(
                                    Button::new(format!("btn-save-{}", id))
                                        .primary()
                                        .xsmall()
                                        .icon(LucideIcon::Check)
                                        .label("Save")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.save_edit(&id, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(format!("btn-cancel-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::X)
                                        .label("Cancel")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.editor.cancel_edit();
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod token_tests {
    use super::{SegmentData, SegmentRow};
    use sona_core::transcription::transcript::{
        TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit,
    };

    fn unit(text: &str, start: f64, end: f64) -> TranscriptTimingUnit {
        TranscriptTimingUnit {
            text: text.to_string(),
            start,
            end,
        }
    }

    #[test]
    fn aligned_units_require_exact_text_and_token_level_timing() {
        let mut data = SegmentData {
            id: "segment-1".to_string(),
            start_time: 0.0,
            end_time: 1.0,
            speaker: String::new(),
            is_final: true,
            speaker_tag: None,
            speaker_attribution: None,
            timing: Some(TranscriptTiming {
                level: TranscriptTimingLevel::Token,
                source: TranscriptTimingSource::Model,
                units: vec![unit("hello ", 0.0, 0.5), unit("world", 0.5, 1.0)],
            }),
            text: "hello world".to_string(),
            translation: None,
        };

        assert_eq!(SegmentRow::aligned_units(&data).unwrap().len(), 2);
        data.text = "edited text".to_string();
        assert!(SegmentRow::aligned_units(&data).is_none());
    }

    #[test]
    fn active_unit_uses_start_inclusive_end_exclusive_boundaries() {
        let units = [unit("one", 0.0, 0.5), unit("two", 0.5, 1.0)];

        assert_eq!(SegmentRow::active_unit_index(&units, 0.0), Some(0));
        assert_eq!(SegmentRow::active_unit_index(&units, 0.5), Some(1));
        assert_eq!(SegmentRow::active_unit_index(&units, 1.0), Some(1));
        assert_eq!(SegmentRow::active_unit_index(&units, -0.1), None);
    }
}

fn render_speaker_correction_menu(
    data: &SegmentData,
    cx: &mut Context<TranscriptWorkbench>,
) -> impl IntoElement {
    let (project_id, profiles, show_all, busy) = {
        let workbench = cx.entity().read(cx);
        (
            workbench.current_project_id.clone(),
            workbench.speaker_correction_profiles.clone(),
            workbench.speaker_menu_show_all,
            workbench.speaker_correction_busy,
        )
    };
    let candidates = data
        .speaker_attribution
        .as_ref()
        .map(|attribution| attribution.candidates.clone())
        .unwrap_or_default();
    let group_id = data
        .speaker_attribution
        .as_ref()
        .map(|attribution| attribution.group_id.clone())
        .filter(|id| !id.is_empty())
        .or_else(|| data.speaker_tag.as_ref().map(|speaker| speaker.id.clone()))
        .unwrap_or_default();
    let primary = profiles
        .iter()
        .filter(|profile| profile.enabled && profile_in_project(profile, project_id.as_deref()))
        .cloned()
        .collect::<Vec<_>>();
    let primary_options = primary
        .into_iter()
        .map(|profile| speaker_profile_option(profile, &group_id, busy, cx).into_any_element())
        .collect::<Vec<_>>();
    let has_primary = !primary_options.is_empty();
    let secondary = profiles
        .iter()
        .filter(|profile| !profile.enabled || !profile_in_project(profile, project_id.as_deref()))
        .cloned()
        .collect::<Vec<_>>();
    let secondary_options = secondary
        .into_iter()
        .map(|profile| speaker_profile_option(profile, &group_id, busy, cx).into_any_element())
        .collect::<Vec<_>>();
    let has_secondary = !secondary_options.is_empty();
    let can_reset = data
        .speaker_attribution
        .as_ref()
        .is_some_and(|attribution| attribution.state != "anonymous");
    let no_profiles = !has_primary && candidates.is_empty() && !has_secondary;
    let segment_id = data.id.clone();

    div()
        .id(ElementId::Name(format!("speaker-menu-{}", data.id).into()))
        .flex()
        .flex_col()
        .gap_1()
        .w(px(280.0))
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_lg()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Assign speaker profile"),
                )
                .child(
                    Button::new(format!("speaker-menu-close-{segment_id}"))
                        .ghost()
                        .xsmall()
                        .icon(LucideIcon::X)
                        .accessibility_label("Close speaker profile menu")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.speaker_menu_open_for = None;
                            this.speaker_menu_show_all = false;
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child("Applies to matching speaker segments in this transcript."),
        )
        .children(candidates.into_iter().map(|candidate| {
            let group_id = group_id.clone();
            Button::new(format!(
                "speaker-candidate-{}-{}",
                segment_id, candidate.profile_id
            ))
            .ghost()
            .small()
            .label(candidate.profile_name)
            .disabled(busy)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.apply_speaker_profile(&group_id, &candidate.profile_id, cx)
            }))
        }))
        .children(primary_options)
        .when(can_reset, |menu| {
            menu.child(
                Button::new(format!("speaker-reset-{segment_id}"))
                    .ghost()
                    .small()
                    .label("Restore anonymous label")
                    .disabled(busy)
                    .on_click(cx.listener({
                        let group_id = group_id.clone();
                        move |this, _, _, cx| this.reset_speaker_group(&group_id, cx)
                    })),
            )
        })
        .when(data.is_final && data.speaker_tag.is_some(), |menu| {
            let segment_id = segment_id.clone();
            menu.child(
                Button::new(format!("speaker-enroll-sample-{segment_id}"))
                    .ghost()
                    .small()
                    .icon(LucideIcon::Mic)
                    .label("Enroll as Voice Sample")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_speaker_enrollment(&segment_id, window, cx)
                    })),
            )
        })
        .when(has_secondary, |menu| {
            menu.child(
                Button::new(format!("speaker-more-{segment_id}"))
                    .ghost()
                    .small()
                    .label(if show_all {
                        "Hide more profiles"
                    } else {
                        "Show all speaker profiles"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.speaker_menu_show_all = !this.speaker_menu_show_all;
                        cx.notify();
                    })),
            )
            .when(show_all, |menu| menu.children(secondary_options))
        })
        .when(no_profiles, |menu| {
            menu.child(
                div()
                    .p_2()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("No speaker profiles yet. Add them in Settings > Vocabulary."),
            )
        })
}

fn profile_in_project(profile: &SpeakerProfile, project_id: Option<&str>) -> bool {
    profile.scope != "project"
        || project_id.is_some_and(|id| {
            profile
                .project_ids
                .iter()
                .any(|project_id| project_id == id)
        })
}

fn speaker_profile_option(
    profile: SpeakerProfile,
    group_id: &str,
    busy: bool,
    cx: &mut Context<TranscriptWorkbench>,
) -> impl IntoElement {
    let profile_id = profile.id;
    let label = profile.name;
    let group_id = group_id.to_string();
    Button::new(format!("speaker-profile-{profile_id}"))
        .ghost()
        .small()
        .label(label)
        .disabled(busy)
        .on_click(
            cx.listener(move |this, _, _, cx| {
                this.apply_speaker_profile(&group_id, &profile_id, cx)
            }),
        )
}

#[cfg(test)]
mod tests {
    use super::profile_in_project;
    use sona_core::transcription::speaker::SpeakerProfile;

    fn profile(scope: &str, project_ids: &[&str]) -> SpeakerProfile {
        SpeakerProfile {
            id: "profile-1".to_string(),
            name: "Speaker".to_string(),
            enabled: true,
            scope: scope.to_string(),
            project_ids: project_ids.iter().map(|id| (*id).to_string()).collect(),
            samples: Vec::new(),
        }
    }

    #[test]
    fn global_profiles_are_available_in_every_project() {
        let global = profile("global", &[]);
        assert!(profile_in_project(&global, None));
        assert!(profile_in_project(&global, Some("project-1")));
    }

    #[test]
    fn project_profiles_match_only_their_assigned_project() {
        let project = profile("project", &["project-1"]);
        assert!(profile_in_project(&project, Some("project-1")));
        assert!(!profile_in_project(&project, Some("project-2")));
        assert!(!profile_in_project(&project, None));
    }
}
