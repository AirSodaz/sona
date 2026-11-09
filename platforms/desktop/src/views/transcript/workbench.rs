//! Transcript Workbench view combining Editor, Player, Toolbar, and Sheets.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::SliderEvent;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::ports::llm::LlmTextGeneratorPort;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum WorkbenchEvent {
    Export {
        title: String,
        has_translation: bool,
        segments: Vec<TranscriptSegment>,
    },
    Closed,
    ItemRenamed {
        id: String,
        title: String,
    },
    SpeakerSampleEnrolled {
        profile_name: String,
    },
}

impl EventEmitter<WorkbenchEvent> for TranscriptWorkbench {}

fn format_segment_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    let minutes = (seconds / 60.0).floor() as u64;
    let whole_seconds = (seconds % 60.0).floor() as u64;
    let tenths = ((seconds % 1.0) * 10.0).floor() as u64;
    format!("{minutes:02}:{whole_seconds:02}.{tenths}")
}
use super::audio_player::AudioPlayerBar;
use super::editor::TranscriptEditor;
use super::search_overlay::SearchOverlay;
use super::speaker_review_sheet::SpeakerReviewSheet;
use super::summary_sheet::SummarySheet;
use super::toolbar::EditorToolbar;
use super::version_sheet::VersionSheet;
use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{
    COLOR_BG_PRIMARY_LIGHT, COLOR_TEXT_MUTED_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_LG,
    RADIUS_MD, RADIUS_SM,
};
use sona_core::history::TranscriptSnapshotReason;
use sona_core::history::mutation_repository::{
    HistoryCommitTranscriptEditRequest, HistoryCommitTranscriptEditResult, HistoryItemMetaPatch,
    HistoryUpdateItemMetaRequest,
};
use sona_core::transcription::speaker::SpeakerProfile;
use sona_core::transcription::speaker_correction::{
    ApplySpeakerProfileToGroupRequest, SpeakerGroupRequest, apply_speaker_profile_to_group_impl,
    reset_speaker_group_to_anonymous_impl,
};
use sona_core::transcription::transcript::TranscriptSegment;
use uuid::Uuid;

pub struct TranscriptWorkbench {
    pub session_title: String,
    pub is_loading: bool,
    pub load_error: bool,
    pub has_session: bool,
    pub history_id: Option<String>,
    pub base_segments: Vec<TranscriptSegment>,
    pub current_project_id: Option<String>,
    pub speaker_correction_profiles: Vec<SpeakerProfile>,
    pub speaker_menu_open_for: Option<String>,
    pub speaker_menu_show_all: bool,
    pub speaker_correction_busy: bool,
    pub speaker_enroll_segment: Option<TranscriptSegment>,
    pub speaker_enroll_mode_new: bool,
    pub speaker_enroll_profile_id: Option<String>,
    pub speaker_enroll_new_name_state: Entity<InputState>,
    pub speaker_enroll_sample_name_state: Entity<InputState>,
    pub speaker_enroll_busy: bool,
    pub speaker_enroll_error: Option<String>,
    pub current_audio_path: Option<std::path::PathBuf>,
    pub editor: TranscriptEditor,
    pub toolbar: EditorToolbar,
    pub player: AudioPlayerBar,
    _player_subscriptions: Vec<Subscription>,
    pub search: SearchOverlay,
    pub summary_sheet: SummarySheet,
    pub speaker_sheet: SpeakerReviewSheet,
    pub version_sheet: VersionSheet,

    pub is_rename_open: bool,
    pub rename_title_state: Entity<InputState>,
    pub rename_icon: Option<String>,
    pub is_ai_renaming: bool,
    pub session_icon: Option<String>,
    pub is_batch_item: bool,
    pub is_streaming_active: bool,
    window_handle: AnyWindowHandle,
    audio_events_task_started: bool,

    // Polish interactive state
    pub is_polishing: bool,
    pub polish_menu_open: bool,
    pub polish_preset: String,
    pub polish_error: Option<String>,
    pub undo_segments: Option<Vec<super::segment_row::SegmentData>>,
    pub redo_segments: Option<Vec<super::segment_row::SegmentData>>,

    // Translate interactive state
    pub is_translating: bool,
    pub translate_menu_open: bool,
    pub show_translations: bool,
    pub translation_source_lang: String,
    pub translation_target_lang: String,
    pub translate_error: Option<String>,

    // Media Server (Emby/Jellyfin) integration
    pub media_server_context: Option<crate::views::create::batch_import::MediaServerTaskContext>,
    pub is_syncing_media_server: bool,
    pub media_server_sync_success: bool,
}

impl TranscriptWorkbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rename_title_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Transcript title"));
        let speaker_enroll_new_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Speaker name"));
        let speaker_enroll_sample_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Clear speech segment"));
        let player = AudioPlayerBar::new(cx);
        let seek_slider = player.seek_slider().clone();
        let volume_slider = player.volume_slider().clone();
        let seek_subscription = cx.subscribe(&seek_slider, |this, _, event: &SliderEvent, cx| {
            if let SliderEvent::Change(value) = event {
                let position = this.player.total_duration * f64::from(value.end());
                this.player.seek_to(position);
                this.editor
                    .update_playback(this.player.is_playing, true, position);
                cx.notify();
            }
        });
        let volume_subscription =
            cx.subscribe(&volume_slider, |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Change(value) = event {
                    this.player.set_volume(value.end());
                    cx.notify();
                }
            });
        Self {
            session_title: String::new(),
            is_loading: false,
            load_error: false,
            has_session: false,
            history_id: None,
            base_segments: Vec::new(),
            current_project_id: None,
            speaker_correction_profiles: Vec::new(),
            speaker_menu_open_for: None,
            speaker_menu_show_all: false,
            speaker_correction_busy: false,
            speaker_enroll_segment: None,
            speaker_enroll_mode_new: false,
            speaker_enroll_profile_id: None,
            speaker_enroll_new_name_state,
            speaker_enroll_sample_name_state,
            speaker_enroll_busy: false,
            speaker_enroll_error: None,
            current_audio_path: None,
            editor: TranscriptEditor::new(window, cx),
            toolbar: EditorToolbar::new(),
            player,
            _player_subscriptions: vec![seek_subscription, volume_subscription],
            search: SearchOverlay::new(window, cx),
            summary_sheet: SummarySheet::new(window, cx),
            speaker_sheet: SpeakerReviewSheet::new(),
            version_sheet: VersionSheet::new(),

            is_rename_open: false,
            rename_title_state,
            rename_icon: None,
            is_ai_renaming: false,
            session_icon: None,
            is_batch_item: false,
            is_streaming_active: false,
            window_handle: window.window_handle(),
            audio_events_task_started: false,
            is_polishing: false,
            polish_menu_open: false,
            polish_preset: "standard".to_string(),
            polish_error: None,
            undo_segments: None,
            redo_segments: None,
            is_translating: false,
            translate_menu_open: false,
            show_translations: true,
            translation_source_lang: "auto".to_string(),
            translation_target_lang: "en".to_string(),
            translate_error: None,
            media_server_context: None,
            is_syncing_media_server: false,
            media_server_sync_success: false,
        }
    }

    pub fn begin_loading(
        &mut self,
        history_id: String,
        title: String,
        icon: Option<String>,
        is_batch: bool,
        project_id: Option<String>,
    ) {
        self.player.reset();
        self.editor.reset_session();
        self.is_streaming_active = false;
        self.history_id = Some(history_id);
        self.current_project_id = project_id;
        self.speaker_menu_open_for = None;
        self.speaker_menu_show_all = false;
        self.speaker_enroll_segment = None;
        self.current_audio_path = None;
        self.session_title = title;
        self.session_icon = icon.clone();
        self.rename_icon = icon;
        self.is_batch_item = is_batch;
        self.is_loading = true;
        self.load_error = false;
        self.has_session = false;
        self.polish_menu_open = false;
        self.translate_menu_open = false;
        self.undo_segments = None;
        self.redo_segments = None;
    }

    pub fn has_translation(&self) -> bool {
        self.editor.segments.iter().any(|s| s.translation.is_some())
    }

    pub fn undo_polish(&mut self, cx: &mut Context<Self>) {
        if let Some(undo) = self.undo_segments.take() {
            self.redo_segments = Some(self.editor.segments.clone());
            self.editor.segments = undo;
            self.persist_current(cx);
            self.polish_menu_open = false;
            cx.notify();
        }
    }

    pub fn redo_polish(&mut self, cx: &mut Context<Self>) {
        if let Some(redo) = self.redo_segments.take() {
            self.undo_segments = Some(self.editor.segments.clone());
            self.editor.segments = redo;
            self.persist_current(cx);
            self.polish_menu_open = false;
            cx.notify();
        }
    }

    pub fn resync_media_server(&mut self, cx: &mut Context<Self>) {
        let Some(ctx) = self.media_server_context.clone() else {
            return;
        };
        let segments = self.current_segments();
        if segments.is_empty() {
            return;
        }
        let srt_text = match sona_core::export::export_segments_with_mode(
            &segments,
            sona_core::export::ExportFormat::Srt,
            sona_core::export::ExportMode::Original,
        ) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to export SRT for media server resync: {e}");
                return;
            }
        };

        self.is_syncing_media_server = true;
        self.media_server_sync_success = false;
        cx.notify();

        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let delete_index = ctx.existing_external_subtitle_index;
        cx.spawn(async move |this, cx| {
            let payload = crate::integrations::media_server::MediaServerSubtitleUploadPayload {
                item_id: ctx.item_id.clone(),
                language: ctx.target_language.clone(),
                format: "srt".to_string(),
                is_forced: false,
                content: srt_text,
            };
            let result = services
                .media_server
                .upload_subtitle(Some(&ctx.profile_id), payload, delete_index)
                .await;
            this.update(cx, |this, cx| {
                this.is_syncing_media_server = false;
                this.media_server_sync_success = result.is_ok();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn set_transcript(&mut self, segments: Vec<TranscriptSegment>, duration: f64) {
        self.base_segments = segments.clone();
        let mut speakers = Vec::new();
        for segment in &segments {
            if let Some(speaker) = &segment.speaker
                && !speaker.label.trim().is_empty()
                && !speakers
                    .iter()
                    .any(|(id, _): &(String, String)| id == &speaker.id)
            {
                speakers.push((speaker.id.clone(), speaker.label.clone()));
            }
        }
        self.speaker_sheet.speakers = speakers;
        self.editor.segments = segments
            .into_iter()
            .map(|segment| super::segment_row::SegmentData {
                id: segment.id,
                start_time: segment.start,
                end_time: segment.end,
                is_final: segment.is_final,
                speaker: segment
                    .speaker
                    .as_ref()
                    .map(|speaker| speaker.label.clone())
                    .unwrap_or_default(),
                speaker_tag: segment.speaker,
                speaker_attribution: segment.speaker_attribution,
                timing: segment.timing,
                text: segment.text,
                translation: segment.translation,
            })
            .collect();
        self.search.invalidate_matches();
        self.editor.set_streaming_active(self.is_streaming_active);
        self.editor.follow_new_segments();
        self.player.current_time = 0.0;
        self.player.total_duration = duration.max(0.0);
        self.player.is_playing = false;
        self.is_loading = false;
        self.load_error = false;
        self.has_session = true;
    }

    pub fn load_audio(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.current_audio_path = Some(path.clone());
        self.player.load(path);
        if self.audio_events_task_started {
            return;
        }
        self.audio_events_task_started = true;
        let window_handle = self.window_handle;
        let events = self.player.events();
        cx.spawn(async move |this, cx| {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut pending = Vec::new();
                while let Ok(event) = events.try_recv() {
                    pending.push(event);
                }
                let keep_running = cx
                    .update_window(window_handle, |_, window, app| {
                        this.update(app, |this, app| {
                            if pending.is_empty() {
                                return true;
                            }
                            for event in pending {
                                if let Some((playing, position)) = this.player.apply_event(event) {
                                    this.player.sync_seek_slider(window, app);
                                    this.editor.update_playback(playing, false, position);
                                }
                            }
                            app.notify();
                            true
                        })
                    })
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn current_segments(&self) -> Vec<TranscriptSegment> {
        self.editor
            .segments
            .iter()
            .map(|data| {
                let mut segment = self
                    .base_segments
                    .iter()
                    .find(|segment| segment.id == data.id)
                    .cloned()
                    .unwrap_or_else(|| TranscriptSegment {
                        id: data.id.clone(),
                        text: String::new(),
                        start: data.start_time,
                        end: data.end_time,
                        is_final: data.is_final,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        draft_translation: None,
                        speaker: None,
                        speaker_attribution: None,
                    });
                segment.text = data.text.clone();
                segment.start = data.start_time;
                segment.end = data.end_time;
                segment.timing = data.timing.clone();
                if data.timing.is_none() {
                    segment.tokens = None;
                    segment.timestamps = None;
                    segment.durations = None;
                }
                segment.translation = data.translation.clone();
                segment.speaker = data.speaker_tag.clone();
                segment.speaker_attribution = data.speaker_attribution.clone();
                segment
            })
            .collect()
    }

    pub fn apply_speaker_profile(
        &mut self,
        group_id: &str,
        profile_id: &str,
        cx: &mut Context<Self>,
    ) {
        let profiles = self.speaker_correction_profiles.clone();
        let enabled_ids = profiles
            .iter()
            .filter(|profile| profile.enabled)
            .map(|profile| profile.id.clone())
            .collect();
        let Ok(response) = apply_speaker_profile_to_group_impl(ApplySpeakerProfileToGroupRequest {
            segments: self.current_segments(),
            group_id: group_id.to_string(),
            target_profile_id: profile_id.to_string(),
            speaker_profiles: profiles.clone(),
            enabled_speaker_profile_ids: enabled_ids,
        }) else {
            return;
        };
        self.apply_speaker_correction_segments(response.segments);
        if let Some(enabled_ids) = response.enabled_speaker_profile_ids {
            let enabled = enabled_ids
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            for profile in &mut self.speaker_correction_profiles {
                profile.enabled = enabled.contains(&profile.id);
                if profile.id == profile_id
                    && profile.scope == "project"
                    && let Some(project_id) = &self.current_project_id
                    && !profile.project_ids.contains(project_id)
                {
                    profile.project_ids.push(project_id.clone());
                }
            }
            let services = cx.global::<GlobalDesktopServices>().0.clone();
            if let Ok(value) = serde_json::to_value(&self.speaker_correction_profiles) {
                let _ = services.config.set_setting("speakerProfiles", &value);
            }
        }
        self.speaker_menu_open_for = None;
        self.speaker_menu_show_all = false;
        self.persist_current(cx);
        cx.notify();
    }

    pub fn reset_speaker_group(&mut self, group_id: &str, cx: &mut Context<Self>) {
        let Ok(response) = reset_speaker_group_to_anonymous_impl(SpeakerGroupRequest {
            segments: self.current_segments(),
            group_id: group_id.to_string(),
        }) else {
            return;
        };
        self.apply_speaker_correction_segments(response.segments);
        self.speaker_menu_open_for = None;
        self.speaker_menu_show_all = false;
        self.persist_current(cx);
        cx.notify();
    }

    pub fn open_speaker_enrollment(
        &mut self,
        segment_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(segment) = self
            .current_segments()
            .into_iter()
            .find(|segment| segment.id == segment_id)
        else {
            return;
        };
        let matching_profile = self.speaker_correction_profiles.iter().find(|profile| {
            profile.id
                == segment
                    .speaker
                    .as_ref()
                    .map(|speaker| speaker.id.as_str())
                    .unwrap_or_default()
                || profile.name.eq_ignore_ascii_case(
                    segment
                        .speaker
                        .as_ref()
                        .map(|speaker| speaker.label.as_str())
                        .unwrap_or_default(),
                )
        });
        let existing_profiles = !self.speaker_correction_profiles.is_empty();
        self.speaker_enroll_mode_new = !existing_profiles;
        self.speaker_enroll_profile_id = matching_profile
            .map(|profile| profile.id.clone())
            .or_else(|| {
                self.speaker_correction_profiles
                    .first()
                    .map(|profile| profile.id.clone())
            });
        let speaker_name = segment
            .speaker
            .as_ref()
            .map(|speaker| speaker.label.clone())
            .unwrap_or_default();
        self.speaker_enroll_new_name_state
            .update(cx, |state, cx| state.set_value(speaker_name, window, cx));
        let sample_name = format!(
            "{} - {}",
            format_segment_time(segment.start),
            format_segment_time(segment.end)
        );
        self.speaker_enroll_sample_name_state
            .update(cx, |state, cx| state.set_value(sample_name, window, cx));
        self.speaker_enroll_error = None;
        self.speaker_enroll_segment = Some(segment);
        self.speaker_menu_open_for = None;
        self.speaker_menu_show_all = false;
        cx.notify();
    }

    pub fn close_speaker_enrollment(&mut self, cx: &mut Context<Self>) {
        if self.speaker_enroll_busy {
            return;
        }
        self.speaker_enroll_segment = None;
        self.speaker_enroll_error = None;
        cx.notify();
    }

    pub fn submit_speaker_enrollment(&mut self, cx: &mut Context<Self>) {
        if self.speaker_enroll_busy {
            return;
        }
        let Some(segment) = self.speaker_enroll_segment.clone() else {
            return;
        };
        let Some(audio_path) = self.current_audio_path.clone() else {
            self.speaker_enroll_error = Some("No audio file found for the current session.".into());
            cx.notify();
            return;
        };
        let is_new = self.speaker_enroll_mode_new;
        let profile_name = self
            .speaker_enroll_new_name_state
            .read(cx)
            .value()
            .trim()
            .to_string();
        if is_new && profile_name.is_empty() {
            self.speaker_enroll_error = Some("Please enter a speaker name.".into());
            cx.notify();
            return;
        }
        let profile_id = if is_new {
            Uuid::new_v4().to_string()
        } else if let Some(profile_id) = &self.speaker_enroll_profile_id {
            if self
                .speaker_correction_profiles
                .iter()
                .any(|profile| profile.id == *profile_id)
            {
                profile_id.clone()
            } else {
                self.speaker_enroll_error = Some("Please select a target speaker profile.".into());
                cx.notify();
                return;
            }
        } else {
            self.speaker_enroll_error = Some("Please select a target speaker profile.".into());
            cx.notify();
            return;
        };
        let sample_name = self
            .speaker_enroll_sample_name_state
            .read(cx)
            .value()
            .trim()
            .to_string();
        let sample_name = (!sample_name.is_empty()).then_some(sample_name);
        let start = segment.start;
        let end = segment.end;
        self.speaker_enroll_busy = true;
        self.speaker_enroll_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
            let result = crate::platform::models::speaker_processing::enroll_speaker_profile_sample_from_audio(
                &provider,
                profile_id.clone(),
                audio_path.to_string_lossy().into_owned(),
                start,
                end,
                sample_name,
            ).await;
            this.update(cx, |this, cx| {
                this.speaker_enroll_busy = false;
                match result {
                    Ok(sample) => {
                        let mut profiles = this.speaker_correction_profiles.clone();
                        if is_new {
                            profiles.push(SpeakerProfile {
                                id: profile_id.clone(),
                                name: profile_name.clone(),
                                enabled: true,
                                scope: "global".to_string(),
                                project_ids: Vec::new(),
                                samples: vec![sample],
                            });
                        } else if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == profile_id) {
                            profile.samples.push(sample);
                        }
                        this.speaker_correction_profiles = profiles;
                        let services = cx.global::<GlobalDesktopServices>().0.clone();
                        if let Ok(value) = serde_json::to_value(&this.speaker_correction_profiles) {
                            let _ = services.config.set_setting("speakerProfiles", &value);
                        }
                        let group_id = segment.speaker_attribution.as_ref().map(|attribution| attribution.group_id.clone())
                            .filter(|group_id| !group_id.is_empty())
                            .or_else(|| segment.speaker.as_ref().map(|speaker| speaker.id.clone()));
                        this.speaker_enroll_segment = None;
                        if let Some(group_id) = group_id {
                            this.apply_speaker_profile(&group_id, &profile_id, cx);
                        }
                        let enrolled_name = if is_new {
                            profile_name.clone()
                        } else {
                            this.speaker_correction_profiles
                                .iter()
                                .find(|profile| profile.id == profile_id)
                                .map(|profile| profile.name.clone())
                                .unwrap_or_default()
                        };
                        cx.emit(WorkbenchEvent::SpeakerSampleEnrolled {
                            profile_name: enrolled_name,
                        });
                        cx.notify();
                    }
                    Err(error) => {
                        this.speaker_enroll_error = Some(error);
                        cx.notify();
                    }
                }
            }).ok();
        }).detach();
    }

    fn apply_speaker_correction_segments(&mut self, segments: Vec<TranscriptSegment>) {
        for segment in segments {
            if let Some(data) = self
                .editor
                .segments
                .iter_mut()
                .find(|data| data.id == segment.id)
            {
                data.speaker = segment
                    .speaker
                    .as_ref()
                    .map(|speaker| speaker.label.clone())
                    .unwrap_or_default();
                data.speaker_tag = segment.speaker;
                data.speaker_attribution = segment.speaker_attribution;
            }
        }
    }

    fn persist_current(&mut self, cx: &mut Context<Self>) {
        self.search.invalidate_matches();
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let base_segments = self.base_segments.clone();
        let edited_segments = self.current_segments();
        let edited_for_commit = edited_segments.clone();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.toolbar.is_saving = true;
        cx.spawn(async move |this, cx| {
            let result = services
                .history
                .mutation_db(move |service| {
                    service.commit_transcript_edit(HistoryCommitTranscriptEditRequest {
                        history_id,
                        edit_session_id: Uuid::new_v4().to_string(),
                        base_segments,
                        edited_segments: edited_for_commit,
                        reason: None,
                    })
                })
                .await;
            this.update(cx, |this, cx| {
                this.toolbar.is_saving = false;
                if matches!(
                    result,
                    Ok(HistoryCommitTranscriptEditResult::Committed { .. }
                        | HistoryCommitTranscriptEditResult::Unchanged)
                ) {
                    this.base_segments = edited_segments;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn begin_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.begin_edit(id, window, cx);
        self.editor
            .update_playback(self.player.is_playing, false, self.player.current_time);
    }

    pub fn save_edit(&mut self, id: &str, cx: &mut Context<Self>) {
        self.editor.save_edit(id, cx);
        self.persist_current(cx);
    }

    pub fn persist_editor_change(&mut self, cx: &mut Context<Self>) {
        self.persist_current(cx);
    }

    pub fn delete_segment(&mut self, id: &str, cx: &mut Context<Self>) {
        self.editor.delete_segment(id);
        self.persist_current(cx);
    }

    pub fn merge_with_next(&mut self, id: &str, cx: &mut Context<Self>) {
        self.editor.merge_with_next(id);
        self.persist_current(cx);
    }

    pub fn split_segment(&mut self, id: &str, cx: &mut Context<Self>) {
        self.editor.split_segment(id);
        self.persist_current(cx);
    }

    pub fn set_load_error(&mut self) {
        self.is_loading = false;
        self.load_error = true;
        self.has_session = false;
    }

    pub fn open_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.session_title.clone();
        self.rename_icon = self.session_icon.clone();
        self.rename_title_state.update(cx, |input, cx| {
            input.set_value(&title, window, cx);
        });
        self.is_rename_open = true;
        cx.notify();
    }

    pub fn save_rename(&mut self, cx: &mut Context<Self>) {
        let new_title = self.rename_title_state.read(cx).value().trim().to_string();
        if new_title.is_empty() {
            return;
        }
        self.session_title = new_title.clone();
        self.session_icon = self.rename_icon.clone();
        self.is_rename_open = false;
        if let Some(history_id) = self.history_id.clone() {
            let services = cx.global::<GlobalDesktopServices>().0.clone();
            let new_icon = self.rename_icon.clone();
            cx.spawn(async move |this, cx| {
                let hid = history_id.clone();
                let title = new_title.clone();
                let _ = services
                    .history
                    .mutation_db(move |mutator| {
                        mutator.update_item_meta(HistoryUpdateItemMetaRequest {
                            history_id: hid,
                            updates: HistoryItemMetaPatch {
                                title: Some(title),
                                icon: Some(new_icon),
                                ..Default::default()
                            },
                        })
                    })
                    .await;
                this.update(cx, |_this, cx| {
                    cx.emit(WorkbenchEvent::ItemRenamed {
                        id: history_id,
                        title: new_title,
                    });
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    pub fn generate_ai_title(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let segments = self.current_segments();
        if segments.is_empty() {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.is_ai_renaming = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let new_title = async {
                let config = services
                    .config
                    .load()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_else(|| serde_json::json!({}));
                let llm_config = crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                    &config,
                    "summary",
                )
                .or_else(|| {
                    crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                        &config,
                        "polish",
                    )
                })
                .ok_or_else(|| "No LLM configured for title generation".to_string())?;

                let sample_text = segments
                    .iter()
                    .take(10)
                    .map(|s| s.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let prompt = format!(
                    "Generate a concise title (under 6 words, no quotes) for this transcript:\n\n{sample_text}\n\nTitle:"
                );

                let data_dir = crate::platform::storage_location::default_app_local_data_dir();
                let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_path(
                    &data_dir,
                );
                let adapter = crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);

                let request = sona_core::llm::requests::LlmGenerateRequest {
                    config: llm_config,
                    input: prompt,
                    source: None,
                };
                let resp = tokio::time::timeout(
                    Duration::from_secs(30),
                    adapter.generate_text(request),
                )
                .await
                .map_err(|_| "Title generation timed out".to_string())?
                .map_err(|e| e.to_string())?;

                let title = resp.text.trim().trim_matches('"').trim().to_string();
                Ok::<String, String>(title)
            }
            .await;

            this.update(cx, |this, cx| {
                if let Ok(title) = new_title {
                    this.session_title = title;
                }
                this.is_ai_renaming = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn start_polish(&mut self, cx: &mut Context<Self>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let segments = self.current_segments();
        if segments.is_empty() {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.undo_segments = Some(self.editor.segments.clone());
        self.redo_segments = None;
        self.is_polishing = true;
        self.polish_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result: Result<Vec<TranscriptSegment>, String> = async {
                let config = services
                    .config
                    .load()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_else(|| serde_json::json!({}));
                let llm_config =
                    crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                        &config, "polish",
                    )
                    .ok_or_else(|| "No polish LLM is configured".to_string())?;

                let data_dir = crate::platform::storage_location::default_app_local_data_dir();
                let models_dir =
                    crate::platform::storage_location::resolve_active_models_dir_for_path(
                        &data_dir,
                    );
                let adapter =
                    crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);

                let segment_inputs = segments
                    .iter()
                    .map(|s| sona_core::llm::tasks::LlmSegmentInput {
                        id: s.id.clone(),
                        text: s.text.clone(),
                    })
                    .collect();

                let request = sona_core::llm::requests::PolishSegmentsRequest {
                    task_id: format!("polish-{}", Uuid::new_v4()),
                    config: llm_config,
                    segments: segment_inputs,
                    chunk_size: None,
                    context: None,
                    keywords: None,
                    mode: None,
                };

                let polished = tokio::time::timeout(
                    Duration::from_secs(120),
                    sona_application::llm::LlmTaskService::new(adapter).polish(request, &()),
                )
                .await
                .map_err(|_| "Polish generation timed out".to_string())?
                .map_err(|error| error.to_string())?;

                let mut updated_segments = segments.clone();
                for (i, p_seg) in polished.into_iter().enumerate() {
                    if let Some(target) = updated_segments.get_mut(i) {
                        target.text = p_seg.text;
                    }
                }

                let hid = history_id.clone();
                let base_segs = segments.clone();
                let edited_segs = updated_segments.clone();
                let _ = services
                    .history
                    .mutation_db(move |mutator| {
                        mutator.commit_transcript_edit(HistoryCommitTranscriptEditRequest {
                            history_id: hid,
                            edit_session_id: Uuid::new_v4().to_string(),
                            base_segments: base_segs,
                            edited_segments: edited_segs,
                            reason: Some(TranscriptSnapshotReason::Polish),
                        })
                    })
                    .await;

                Ok(updated_segments)
            }
            .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(new_segments) => {
                        let duration = this.player.total_duration;
                        this.set_transcript(new_segments, duration);
                    }
                    Err(err) => {
                        this.polish_error = Some(err);
                    }
                }
                this.is_polishing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn start_translate(&mut self, target_lang: &str, cx: &mut Context<Self>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let segments = self.current_segments();
        if segments.is_empty() {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let target_language = target_lang.to_string();
        self.is_translating = true;
        self.translate_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result: Result<Vec<TranscriptSegment>, String> = async {
                let config = services
                    .config
                    .load()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_else(|| serde_json::json!({}));
                let llm_config =
                    crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                        &config,
                        "translation",
                    )
                    .ok_or_else(|| "No translation LLM is configured".to_string())?;

                let data_dir = crate::platform::storage_location::default_app_local_data_dir();
                let models_dir =
                    crate::platform::storage_location::resolve_active_models_dir_for_path(
                        &data_dir,
                    );
                let adapter =
                    crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);

                let segment_inputs = segments
                    .iter()
                    .map(|s| sona_core::llm::tasks::LlmSegmentInput {
                        id: s.id.clone(),
                        text: s.text.clone(),
                    })
                    .collect();

                let request = sona_core::llm::requests::TranslateSegmentsRequest {
                    task_id: format!("translate-{}", Uuid::new_v4()),
                    config: llm_config,
                    segments: segment_inputs,
                    chunk_size: None,
                    target_language: target_language.clone(),
                    target_language_name: None,
                    source_language: None,
                    context: None,
                    keywords: None,
                };

                let translated = tokio::time::timeout(
                    Duration::from_secs(120),
                    sona_application::llm::LlmTaskService::new(adapter).translate(request, &()),
                )
                .await
                .map_err(|_| "Translation timed out".to_string())?
                .map_err(|error| error.to_string())?;

                let mut updated_segments = segments.clone();
                for (i, t_seg) in translated.into_iter().enumerate() {
                    if let Some(target) = updated_segments.get_mut(i) {
                        target.translation = Some(t_seg.translation);
                    }
                }

                let hid = history_id.clone();
                let base_segs = segments.clone();
                let edited_segs = updated_segments.clone();
                let _ = services
                    .history
                    .mutation_db(move |mutator| {
                        mutator.commit_transcript_edit(HistoryCommitTranscriptEditRequest {
                            history_id: hid,
                            edit_session_id: Uuid::new_v4().to_string(),
                            base_segments: base_segs,
                            edited_segments: edited_segs,
                            reason: Some(TranscriptSnapshotReason::Translate),
                        })
                    })
                    .await;

                Ok(updated_segments)
            }
            .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(new_segments) => {
                        let duration = this.player.total_duration;
                        this.set_transcript(new_segments, duration);
                    }
                    Err(err) => {
                        this.translate_error = Some(err);
                    }
                }
                this.is_translating = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn set_versions(&mut self, snapshots: Vec<sona_core::history::TranscriptSnapshotMetadata>) {
        self.version_sheet.versions = snapshots
            .into_iter()
            .map(|snapshot| {
                (
                    format!("{} ({:?})", snapshot.id, snapshot.reason),
                    snapshot.created_at.to_string(),
                )
            })
            .collect();
    }

    pub fn set_summary(&mut self, summary: Option<sona_core::history::HistorySummaryPayload>) {
        if let Some(payload) = summary.as_ref() {
            self.summary_sheet.template = payload.active_template_id.clone();
        }
        self.summary_sheet.summary_text = summary
            .and_then(|payload| payload.record)
            .map(|record| record.content)
            .unwrap_or_default();
    }

    pub fn save_summary_edit(&mut self, cx: &mut Context<Self>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let content = self.summary_sheet.editor_state.read(cx).value().to_string();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.summary_sheet.is_saving = true;
        self.summary_sheet.error_message = None;
        cx.spawn(async move |this, cx| {
            let result = crate::platform::agent_control::facade::history::save_summary(
                &services,
                crate::platform::agent_control::facade::types::SaveSummaryRequest {
                    history_id,
                    content: content.clone(),
                    template_id: None,
                    thought: None,
                },
            )
            .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(_) => this.summary_sheet.summary_text = content,
                    Err(error) => this.summary_sheet.error_message = Some(error),
                }
                this.summary_sheet.is_saving = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn delete_summary(&mut self, cx: &mut Context<Self>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.summary_sheet.is_saving = true;
        self.summary_sheet.error_message = None;
        cx.spawn(async move |this, cx| {
            let result = crate::platform::agent_control::facade::history::delete_summary(
                &services, history_id,
            )
            .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(_) => this.summary_sheet.summary_text.clear(),
                    Err(error) => this.summary_sheet.error_message = Some(error),
                }
                this.summary_sheet.is_saving = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn save_speaker_rename(&mut self, target_id: &str, cx: &mut Context<Self>) {
        let Some(state) = &self.speaker_sheet.edit_name_state else {
            return;
        };
        let new_name = state.read(cx).value().trim().to_string();
        if new_name.is_empty() {
            self.speaker_sheet.cancel_rename(cx);
            return;
        }

        let old_label = self
            .speaker_sheet
            .speakers
            .iter()
            .find(|(id, _)| id == target_id)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| target_id.to_string());

        for (id, name) in &mut self.speaker_sheet.speakers {
            if id == target_id {
                *name = new_name.clone();
            }
        }

        for seg in &mut self.editor.segments {
            if seg.speaker == old_label || seg.speaker == target_id {
                seg.speaker = new_name.clone();
            }
        }

        for seg in &mut self.base_segments {
            if let Some(spk) = &mut seg.speaker
                && (spk.id == target_id || spk.label == old_label)
            {
                spk.label = new_name.clone();
            }
        }

        self.speaker_sheet.cancel_rename(cx);
        self.persist_current(cx);
        cx.notify();
    }

    pub fn refresh_summary(&mut self, cx: &mut Context<Self>) {
        let Some(history_id) = self.history_id.clone() else {
            self.summary_sheet.is_generating = false;
            return;
        };
        let segments = self.current_segments();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.summary_sheet.is_generating = true;
        self.summary_sheet.error_message = None;
        cx.spawn(async move |this, cx| {
            let summary = async {
                let config = services
                    .config
                    .load()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_else(|| serde_json::json!({}));
                let llm_config = crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                    &config,
                    "summary",
                )
                .ok_or_else(|| "No summary LLM is configured".to_string())?;
                let template_id = config
                    .get("summaryTemplateId")
                    .and_then(|value| value.as_str())
                    .unwrap_or("general")
                    .to_string();
                let template = config
                    .get("summaryCustomTemplates")
                    .and_then(|value| value.as_array())
                    .and_then(|templates| {
                        templates.iter().find_map(|template| {
                            (template.get("id").and_then(|value| value.as_str())
                                == Some(template_id.as_str()))
                            .then(|| sona_core::llm::tasks::SummaryTemplateConfig {
                                id: template_id.clone(),
                                name: template
                                    .get("name")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("Custom")
                                    .to_string(),
                                instructions: template
                                    .get("instructions")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            })
                        })
                    })
                    .unwrap_or_else(|| sona_core::llm::tasks::SummaryTemplateConfig {
                        id: "general".to_string(),
                        name: "General".to_string(),
                        instructions: "1. A short overview paragraph.\n2. A concise list of key points.\n3. Follow-up items only when supported by the transcript.".to_string(),
                    });
                if segments.is_empty() {
                    return Err("Transcript cannot be empty".to_string());
                }
                let data_dir = crate::platform::storage_location::default_app_local_data_dir();
                let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_path(&data_dir);
                let adapter = crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);
                let request = sona_core::llm::requests::SummarizeTranscriptRequest {
                    task_id: format!("summary-{}", uuid::Uuid::new_v4()),
                    config: llm_config,
                    template,
                    segments: sona_core::llm::jobs::summary_inputs_from_transcript(&segments),
                    chunk_char_budget: None,
                };
                let result = tokio::time::timeout(
                    Duration::from_secs(120),
                    sona_application::llm::LlmTaskService::new(adapter).summarize(request, &()),
                )
                .await
                .map_err(|_| "Summary generation timed out".to_string())?
                .map_err(|error| error.to_string())?;
                let save_request = crate::platform::agent_control::facade::types::SaveSummaryRequest {
                    history_id: history_id.clone(),
                    content: result.content.clone(),
                    template_id: Some(result.template_id),
                    thought: result.thought,
                };
                crate::platform::agent_control::facade::history::save_summary(&services, save_request)
                    .await?;
                Ok::<String, String>(result.content)
            }
            .await;
            this.update(cx, |this, cx| {
                match summary {
                    Ok(content) => {
                        this.summary_sheet.summary_text = content;
                    }
                    Err(error) => {
                        this.summary_sheet.error_message = Some(error);
                    }
                }
                this.summary_sheet.is_generating = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn restore_version(&mut self, snapshot_id: &str, cx: &mut Context<Self>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let snapshot_id = snapshot_id.to_string();
        let duration = self.player.total_duration;
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx| {
            let result =
                crate::platform::agent_control::facade::transcript::revert_transcript_snapshot(
                    &services,
                    history_id.clone(),
                    snapshot_id,
                )
                .await;
            if result.is_ok()
                && let Ok(Some(segments)) = services
                    .history
                    .query_db(move |query| query.load_transcript(&history_id))
                    .await
            {
                this.update(cx, |this, cx| {
                    this.set_transcript(segments, duration);
                    this.version_sheet.is_open = false;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn render_speaker_enrollment_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(segment) = self.speaker_enroll_segment.clone() else {
            return div().into_any_element();
        };
        let duration = (segment.end - segment.start).max(0.0);
        let profiles = self.speaker_correction_profiles.clone();
        let new_name_state = self.speaker_enroll_new_name_state.clone();
        let sample_name_state = self.speaker_enroll_sample_name_state.clone();
        let selected_profile_id = self.speaker_enroll_profile_id.clone();
        let is_new = self.speaker_enroll_mode_new;
        let busy = self.speaker_enroll_busy;
        let audio_available = self.current_audio_path.is_some();
        let error = self.speaker_enroll_error.clone();

        div()
            .id("speaker-enrollment-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(hsla(0.0, 0.0, 0.0, 0.5))
            .on_click(cx.listener(|this, _, _, cx| this.close_speaker_enrollment(cx)))
            .child(
                div()
                    .id("speaker-enrollment-dialog")
                    .w(px(480.0))
                    .max_w_full()
                    .max_h(px(720.0))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_5()
                    .rounded(RADIUS_LG)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_2xl()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Icon::new(LucideIcon::Mic).size(px(18.0)))
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Enroll Speaker Voice Sample")),
                            )
                            .child(
                                Button::new("speaker-enrollment-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::X)
                                    .accessibility_label("Close enrollment dialog")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.close_speaker_enrollment(cx))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().secondary)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(12.0))
                                    .child(format!("{} - {}", format_segment_time(segment.start), format_segment_time(segment.end)))
                                    .child(format!("{duration:.1}s")),
                            )
                            .child(div().text_size(px(13.0)).child(segment.text.clone())),
                    )
                    .when(duration < 4.0, |dialog| {
                        dialog.child(
                            div()
                                .flex()
                                .items_start()
                                .gap_2()
                                .rounded_md()
                                .p_3()
                                .bg(cx.theme().warning.opacity(0.12))
                                .text_size(px(12.0))
                                .child(Icon::new(LucideIcon::TriangleAlert).size(px(15.0)))
                                .child("This segment is under 4 seconds. The speaker engine ignores shorter samples; choose a longer segment when possible."),
                        )
                    })
                    .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Target Speaker"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("speaker-enrollment-existing-mode")
                                    .when(!is_new, |button| button.primary())
                                    .when(is_new, |button| button.outline())
                                    .small()
                                    .icon(LucideIcon::User)
                                    .label("Existing Profile")
                                    .disabled(profiles.is_empty() || busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.speaker_enroll_mode_new = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("speaker-enrollment-new-mode")
                                    .when(is_new, |button| button.primary())
                                    .when(!is_new, |button| button.outline())
                                    .small()
                                    .icon(LucideIcon::Plus)
                                    .label("New Speaker")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.speaker_enroll_mode_new = true;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(is_new, |dialog| {
                        dialog.child(Input::new(&new_name_state).focus_ring(true))
                    })
                    .when(!is_new, |dialog| {
                        dialog.child(
                            div()
                                .max_h(px(150.0))
                                .overflow_y_scrollbar()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .children(profiles.into_iter().map(|profile| {
                                    let profile_id = profile.id.clone();
                                    Button::new(format!("enroll-profile-{}", profile.id))
                                        .when(selected_profile_id.as_deref() == Some(profile.id.as_str()), |button| button.secondary())
                                        .ghost()
                                        .small()
                                        .justify_start()
                                        .label(format!("{} ({} samples)", profile.name, profile.samples.len()))
                                        .disabled(busy)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.speaker_enroll_profile_id = Some(profile_id.clone());
                                            cx.notify();
                                        }))
                                })),
                        )
                    })
                    .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Sample Label (optional)"))
                    .child(Input::new(&sample_name_state).focus_ring(true))
                    .when(!audio_available || error.is_some(), |dialog| {
                        let message = error.clone().unwrap_or_else(|| {
                            "No audio file found for the current session.".to_string()
                        });
                        dialog.child(div().text_size(px(12.0)).text_color(cx.theme().danger).child(message))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("speaker-enrollment-cancel")
                                    .ghost()
                                    .small()
                                    .label("Cancel")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.close_speaker_enrollment(cx))),
                            )
                            .child(
                                Button::new("speaker-enrollment-submit")
                                    .primary()
                                    .small()
                                    .icon(if busy { LucideIcon::Loader } else { LucideIcon::Mic })
                                    .label("Enroll Voice Sample")
                                    .disabled(busy || !audio_available)
                                    .on_click(cx.listener(|this, _, _, cx| this.submit_speaker_enrollment(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.query_state.read(cx).value().to_string();
        if query != self.search.query {
            self.search.query = query.clone();
            self.search.current_match = 0;
            self.search.invalidate_matches();
        }
        if self.search.matches_dirty {
            self.search.matches =
                super::search_overlay::find_transcript_matches(&self.editor.segments, &query);
            self.search.match_count = self.search.matches.len();
            if self.search.matches.is_empty() {
                self.search.current_match = 0;
                self.editor.search_highlights.clear();
            } else {
                self.search.current_match =
                    self.search.current_match.min(self.search.matches.len() - 1);
                let mut highlights = std::collections::HashMap::new();
                for (global_index, matched) in self.search.matches.iter().enumerate() {
                    highlights
                        .entry(matched.segment_id.clone())
                        .or_insert_with(Vec::new)
                        .push(super::segment_row::SegmentSearchHighlight {
                            range: matched.range.clone(),
                            global_index,
                        });
                }
                self.editor.search_highlights = highlights;
            }
            self.search.matches_dirty = false;
        }
        self.editor.active_search_match =
            (self.search.match_count > 0).then_some(self.search.current_match);
        if !self.has_session {
            if self.is_loading {
                return div()
                    .id("transcript-loading-state")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                    .child("Loading transcript…");
            }
            if self.load_error {
                return div()
                    .id("transcript-error-state")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                    .child("Couldn't load this transcript");
            }
            return div()
                .id("transcript-empty-state")
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_size(px(14.0))
                .text_color(COLOR_TEXT_MUTED_LIGHT)
                .child("Select a transcript to open it");
        }

        if self.editor.segments.is_empty() {
            return div()
                .id("transcript-empty-segments-state")
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .size_full()
                .gap(px(16.0))
                .child(
                    Icon::default()
                        .path("icons/circle-plus.svg")
                        .size(px(40.0))
                        .text_color(COLOR_TEXT_MUTED_LIGHT),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                .child("No transcript segments yet."),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(COLOR_TEXT_MUTED_LIGHT)
                                .child("Start recording or import a file."),
                        ),
                );
        }

        div()
            .id("transcript-workbench-shell")
            .flex()
            .flex_col()
            .size_full()
            .bg(COLOR_BG_PRIMARY_LIGHT)
            .child(
                // Workbench header bar
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(48.0))
                    .w_full()
                    .px_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().title_bar)
                    // Left: Icon + Title + Rename + Summary + Speakers + Versions
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child({
                                let emoji_icon = self.session_icon.as_ref().filter(|ic| !ic.is_empty() && !ic.starts_with("system:"));
                                let system_icon = self.session_icon.as_ref().and_then(|ic| ic.strip_prefix("system:"));
                                if let Some(emoji) = emoji_icon {
                                    div()
                                        .text_size(px(16.0))
                                        .child(emoji.clone())
                                        .into_any_element()
                                } else if let Some(sys) = system_icon {
                                    let lucide = match sys {
                                        "mic" => LucideIcon::Mic,
                                        "file" => LucideIcon::FileText,
                                        "folder" => LucideIcon::Folder,
                                        "code" => LucideIcon::Code,
                                        _ => LucideIcon::FileText,
                                    };
                                    Icon::new(lucide)
                                        .size(px(16.0))
                                        .text_color(cx.theme().muted)
                                        .into_any_element()
                                } else if self.is_batch_item {
                                    Icon::new(LucideIcon::FileText)
                                        .size(px(16.0))
                                        .text_color(cx.theme().muted)
                                        .into_any_element()
                                } else {
                                    Icon::new(LucideIcon::Mic)
                                        .size(px(16.0))
                                        .text_color(cx.theme().muted)
                                        .into_any_element()
                                }
                            })
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.session_title.clone()),
                            )
                            .when_some(self.media_server_context.as_ref(), |this, ctx| {
                                let is_jellyfin = ctx.profile_id.to_lowercase().contains("jellyfin") || ctx.profile_id.starts_with("jf");
                                let tag = if is_jellyfin { "Jellyfin" } else { "Emby" };
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .px_2()
                                        .py(px(2.0))
                                        .rounded_full()
                                        .bg(if is_jellyfin {
                                            hsla(271.0 / 360.0, 0.91, 0.65, 0.12)
                                        } else {
                                            hsla(142.0 / 360.0, 0.71, 0.45, 0.12)
                                        })
                                        .border_1()
                                        .border_color(if is_jellyfin {
                                            hsla(271.0 / 360.0, 0.91, 0.65, 0.25)
                                        } else {
                                            hsla(142.0 / 360.0, 0.71, 0.45, 0.25)
                                        })
                                        .text_color(if is_jellyfin {
                                            hsla(271.0 / 360.0, 0.91, 0.65, 1.0)
                                        } else {
                                            hsla(142.0 / 360.0, 0.71, 0.45, 1.0)
                                        })
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(Icon::new(LucideIcon::Tv).size(px(12.0)))
                                        .child(tag),
                                )
                            })
                            .child(
                                Button::new("btn-rename-workbench")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Pencil)
                                    .tooltip("Rename")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_rename(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-open-summary")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Sparkles)
                                    .label("Summary")
                                    .tooltip("AI Summary")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.summary_sheet.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-open-speakers")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Users)
                                    .label("Speakers")
                                    .tooltip("Speaker Review")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.speaker_sheet.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-open-versions")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Clock)
                                    .label("Versions")
                                    .tooltip("Version History")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.version_sheet.toggle();
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Right: Polish, Translate, Export, Close (✕)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when_some(self.media_server_context.as_ref(), |this, ctx| {
                                let is_jellyfin = ctx.profile_id.to_lowercase().contains("jellyfin") || ctx.profile_id.starts_with("jf");
                                let label = if self.is_syncing_media_server {
                                    "Syncing...".to_string()
                                } else if self.media_server_sync_success {
                                    "Synced".to_string()
                                } else {
                                    format!("Re-sync to {}", if is_jellyfin { "Jellyfin" } else { "Emby" })
                                };
                                this.child(
                                    Button::new("btn-resync-media-server")
                                        .secondary()
                                        .small()
                                        .icon(LucideIcon::Tv)
                                        .label(label)
                                        .disabled(self.is_syncing_media_server || self.editor.segments.is_empty())
                                        .tooltip("Re-sync subtitles to Media Server")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.resync_media_server(cx);
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .relative()
                                    .child(
                                        Button::new("btn-polish-workbench")
                                            .ghost()
                                            .small()
                                            .icon(LucideIcon::Sparkles)
                                            .label(if self.is_polishing {
                                                "Polishing..."
                                            } else {
                                                "Polish"
                                            })
                                            .tooltip("Polish transcript with LLM")
                                            .disabled(self.is_polishing)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.polish_menu_open = !this.polish_menu_open;
                                                this.translate_menu_open = false;
                                                cx.notify();
                                            })),
                                    )
                                    .when(self.polish_menu_open, |this| {
                                        this.child(
                                            div()
                                                .id("polish-menu-dropdown")
                                                .absolute()
                                                .top(px(36.0))
                                                .right_0()
                                                .w(px(240.0))
                                                .p_2()
                                                .rounded(RADIUS_MD)
                                                .bg(cx.theme().background)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .shadow_xl()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    Button::new("btn-start-polish-action")
                                                        .ghost()
                                                        .small()
                                                        .icon(LucideIcon::Sparkles)
                                                        .label("LLM Polish")
                                                        .disabled(self.is_polishing)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.polish_menu_open = false;
                                                            this.start_polish(cx);
                                                        })),
                                                 )
                                                .when(self.undo_segments.is_some(), |menu| {
                                                    menu.child(
                                                        Button::new("btn-undo-polish")
                                                            .ghost()
                                                            .small()
                                                            .icon(LucideIcon::Undo2)
                                                            .label("Undo Polish")
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.undo_polish(cx);
                                                            })),
                                                    )
                                                })
                                                .when(self.redo_segments.is_some(), |menu| {
                                                    menu.child(
                                                        Button::new("btn-redo-polish")
                                                            .ghost()
                                                            .small()
                                                            .icon(LucideIcon::Redo2)
                                                            .label("Redo Polish")
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.redo_polish(cx);
                                                            })),
                                                    )
                                                })
                                                .child(div().h(px(1.0)).bg(cx.theme().border).my_1())
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .text_size(px(11.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().muted)
                                                        .child("POLISH PRESET"),
                                                )
                                                .children(["standard", "fluency", "concise", "meeting"].into_iter().map(|preset| {
                                                    let is_active = self.polish_preset == preset;
                                                    let label = match preset {
                                                        "standard" => "Standard Accuracy",
                                                        "fluency" => "Fluency & Natural",
                                                        "concise" => "Concise Summary",
                                                        "meeting" => "Meeting Cleanup",
                                                        _ => preset,
                                                    };
                                                    let p = preset.to_string();
                                                    Button::new(format!("btn-polish-preset-{}", preset))
                                                        .ghost()
                                                        .small()
                                                        .icon(if is_active { LucideIcon::Check } else { LucideIcon::Sparkles })
                                                        .label(label)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.polish_preset = p.clone();
                                                            this.polish_menu_open = false;
                                                            cx.notify();
                                                        }))
                                                })),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .relative()
                                    .child(
                                        Button::new("btn-translate-workbench")
                                            .ghost()
                                            .small()
                                            .icon(LucideIcon::Languages)
                                            .label(if self.is_translating {
                                                "Translating..."
                                            } else {
                                                "Translate"
                                            })
                                            .tooltip("Translate transcript")
                                            .disabled(self.is_translating)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.translate_menu_open = !this.translate_menu_open;
                                                this.polish_menu_open = false;
                                                cx.notify();
                                            })),
                                    )
                                    .when(self.translate_menu_open, |this| {
                                        this.child(
                                            div()
                                                .id("translate-menu-dropdown")
                                                .absolute()
                                                .top(px(36.0))
                                                .right_0()
                                                .w(px(260.0))
                                                .p_2()
                                                .rounded(RADIUS_MD)
                                                .bg(cx.theme().background)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .shadow_xl()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    Button::new("btn-start-translate-action")
                                                        .ghost()
                                                        .small()
                                                        .icon(LucideIcon::Languages)
                                                        .label(if self.has_translation() { "Retranslate" } else { "Start Translation" })
                                                        .disabled(self.is_translating)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            let target = this.translation_target_lang.clone();
                                                            this.translate_menu_open = false;
                                                            this.start_translate(&target, cx);
                                                        })),
                                                )
                                                .child(
                                                    Button::new("btn-toggle-translations-vis")
                                                        .ghost()
                                                        .small()
                                                        .icon(if self.show_translations { LucideIcon::EyeOff } else { LucideIcon::Eye })
                                                        .label(if self.show_translations { "Hide Translations" } else { "Show Translations" })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.show_translations = !this.show_translations;
                                                            this.editor.show_translations = this.show_translations;
                                                            this.translate_menu_open = false;
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(div().h(px(1.0)).bg(cx.theme().border).my_1())
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .px_2()
                                                        .py_1()
                                                        .child(
                                                            div()
                                                                .text_size(px(11.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(cx.theme().muted)
                                                                .child("TARGET LANGUAGE"),
                                                        )
                                                        .child(
                                                            Button::new("btn-swap-languages")
                                                                .ghost()
                                                                .xsmall()
                                                                .icon(LucideIcon::ArrowLeftRight)
                                                                .tooltip("Swap Source & Target")
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    std::mem::swap(&mut this.translation_source_lang, &mut this.translation_target_lang);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                                .children([
                                                    ("zh", "Chinese (Simplified)"),
                                                    ("en", "English"),
                                                    ("ja", "Japanese"),
                                                    ("ko", "Korean"),
                                                    ("fr", "French"),
                                                    ("de", "German"),
                                                    ("es", "Spanish"),
                                                    ("ru", "Russian"),
                                                ].into_iter().map(|(code, name)| {
                                                    let is_selected = self.translation_target_lang == code;
                                                    let c = code.to_string();
                                                    Button::new(format!("btn-lang-{}", code))
                                                        .ghost()
                                                        .small()
                                                        .icon(if is_selected { LucideIcon::Check } else { LucideIcon::Languages })
                                                        .label(name)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.translation_target_lang = c.clone();
                                                            this.translate_menu_open = false;
                                                            cx.notify();
                                                        }))
                                                })),
                                        )
                                    }),
                            )
                            .child(
                                Button::new("btn-export-workbench")
                                    .primary()
                                    .small()
                                    .icon(LucideIcon::Download)
                                    .label("Export")
                                    .tooltip("Export transcript to file")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        let has_translation = this
                                            .editor
                                            .segments
                                            .iter()
                                            .any(|s| s.translation.is_some());
                                        let segments = this.base_segments.clone();
                                        let title = this.session_title.clone();
                                        cx.emit(WorkbenchEvent::Export {
                                            title,
                                            has_translation,
                                            segments,
                                        });
                                    })),
                            )
                            .child(
                                Button::new("btn-close-workbench")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::X)
                                    .tooltip("Close transcript")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.has_session = false;
                                        this.history_id = None;
                                        cx.emit(WorkbenchEvent::Closed);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                // Formatting toolbar
                self.toolbar
                    .render(self.editor.editing_id.is_some(), window, cx),
            )
            .child(
                // Center split area: Editor + optional drawers
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .h_full()
                            .child(self.editor.render(window, cx)),
                    )
                    .child(self.summary_sheet.render(window, cx))
                    .child(self.speaker_sheet.render(cx))
                    .child(self.version_sheet.render(cx))
                    .child(
                        div()
                            .absolute()
                            .bottom(px(24.0))
                            .left_1_2()
                            .ml(px(-160.0))
                            .child(self.search.render(cx)),
                    ),
            )
            .child(
                // Sticky bottom audio player
                self.player.render(cx),
            )
            .when(self.is_rename_open, |this| {
                this.child(
                    div()
                        .id("rename-modal-backdrop")
                        .absolute()
                        .inset_0()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.is_rename_open = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .id("rename-modal-content")
                                .w(px(400.0))
                                .p_6()
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("Rename Transcript"),
                                )
                                .child({
                                    let current_icon = self.rename_icon.clone().unwrap_or_else(|| {
                                        if self.is_batch_item { "system:file".to_string() } else { "system:mic".to_string() }
                                    });
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap_1()
                                        .p_1()
                                        .rounded(RADIUS_SM)
                                        .bg(cx.theme().secondary)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .children(["system:mic", "system:file", "system:folder", "system:code", "📝", "🎙️", "💡", "🎬"].into_iter().map(|icon_val| {
                                            let is_active = current_icon == icon_val;
                                            let icon_str = icon_val.to_string();
                                            div()
                                                .id(ElementId::from(format!("btn-icon-pick-{}", icon_val)))
                                                .size(px(26.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(4.0))
                                                .cursor_pointer()
                                                .border_1()
                                                .border_color(if is_active { cx.theme().primary } else { gpui_kit::transparent_black() })
                                                .child(if icon_val.starts_with("system:") {
                                                    let sys = icon_val.strip_prefix("system:").unwrap_or("");
                                                    let lucide = match sys {
                                                        "mic" => LucideIcon::Mic,
                                                        "file" => LucideIcon::FileText,
                                                        "folder" => LucideIcon::Folder,
                                                        "code" => LucideIcon::Code,
                                                        _ => LucideIcon::FileText,
                                                    };
                                                    Icon::new(lucide).size(px(14.0)).text_color(if is_active { cx.theme().primary } else { cx.theme().muted }).into_any_element()
                                                } else {
                                                    div().text_size(px(12.0)).child(icon_val).into_any_element()
                                                })
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.rename_icon = Some(icon_str.clone());
                                                    cx.notify();
                                                }))
                                        }))
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().flex_1().child(
                                            Input::new(&self.rename_title_state).focus_ring(true),
                                        ))
                                        .child(
                                            Button::new("btn-ai-rename")
                                                .ghost()
                                                .small()
                                                .icon(if self.is_ai_renaming {
                                                    LucideIcon::Loader
                                                } else {
                                                    LucideIcon::Sparkles
                                                })
                                                .tooltip("AI Auto-rename")
                                                .disabled(self.is_ai_renaming)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.generate_ai_title(window, cx);
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-cancel-rename")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.is_rename_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-save-rename")
                                                .primary()
                                                .small()
                                                .label("Save")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.save_rename(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            })
            .child(self.render_speaker_enrollment_dialog(cx))
    }
}
impl Render for TranscriptWorkbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Self::render(self, window, cx)
    }
}
