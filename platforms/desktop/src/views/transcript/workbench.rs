//! Transcript Workbench view combining Editor, Player, Toolbar, and Sheets.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
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
}

impl EventEmitter<WorkbenchEvent> for TranscriptWorkbench {}
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
use sona_core::transcription::transcript::TranscriptSegment;
use uuid::Uuid;

pub struct TranscriptWorkbench {
    pub session_title: String,
    pub is_loading: bool,
    pub load_error: bool,
    pub has_session: bool,
    pub history_id: Option<String>,
    pub base_segments: Vec<TranscriptSegment>,
    pub editor: TranscriptEditor,
    pub toolbar: EditorToolbar,
    pub player: AudioPlayerBar,
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
        Self {
            session_title: String::new(),
            is_loading: false,
            load_error: false,
            has_session: false,
            history_id: None,
            base_segments: Vec::new(),
            editor: TranscriptEditor::new(window, cx),
            toolbar: EditorToolbar::new(),
            player: AudioPlayerBar::new(),
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
    ) {
        self.history_id = Some(history_id);
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
                speaker: segment
                    .speaker
                    .map(|speaker| speaker.label)
                    .unwrap_or_default(),
                text: segment.text,
                translation: segment.translation,
            })
            .collect();
        self.player.current_time = 0.0;
        self.player.total_duration = duration.max(0.0);
        self.player.is_playing = false;
        self.is_loading = false;
        self.load_error = false;
        self.has_session = true;
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
                        is_final: true,
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
                segment.translation = data.translation.clone();
                segment
            })
            .collect()
    }

    fn persist_current(&mut self, cx: &mut Context<Self>) {
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

    pub fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self
            .search
            .query_state
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        if query.is_empty() {
            self.search.match_count = 0;
            self.search.current_match = 0;
            self.editor.search_match_id = None;
        } else {
            let matches = self
                .editor
                .segments
                .iter()
                .filter(|segment| segment.text.to_lowercase().contains(&query))
                .map(|segment| segment.id.clone())
                .collect::<Vec<_>>();
            self.search.match_count = matches.len();
            if self.search.match_count == 0 {
                self.search.current_match = 0;
                self.editor.search_match_id = None;
            } else {
                self.search.current_match =
                    self.search.current_match.min(self.search.match_count - 1);
                self.editor.search_match_id = matches.get(self.search.current_match).cloned();
            }
        }
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
    }
}
impl Render for TranscriptWorkbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Self::render(self, window, cx)
    }
}
