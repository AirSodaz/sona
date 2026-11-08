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

use super::super::shell::MainShell;
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
    pub is_ai_renaming: bool,
    pub is_polishing: bool,
    pub polish_error: Option<String>,
    pub is_translating: bool,
    pub translate_error: Option<String>,
}

impl TranscriptWorkbench {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
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
            is_ai_renaming: false,
            is_polishing: false,
            polish_error: None,
            is_translating: false,
            translate_error: None,
        }
    }

    pub fn begin_loading(&mut self, history_id: String, title: String) {
        self.history_id = Some(history_id);
        self.session_title = title;
        self.is_loading = true;
        self.load_error = false;
        self.has_session = false;
    }

    pub fn set_transcript(&mut self, segments: Vec<TranscriptSegment>, duration: f64) {
        self.base_segments = segments.clone();
        let mut speakers = Vec::new();
        for segment in &segments {
            if let Some(speaker) = &segment.speaker {
                if !speaker.label.trim().is_empty()
                    && !speakers
                        .iter()
                        .any(|(id, _): &(String, String)| id == &speaker.id)
                {
                    speakers.push((speaker.id.clone(), speaker.label.clone()));
                }
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

    fn persist_current(&mut self, cx: &mut Context<MainShell>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let base_segments = self.base_segments.clone();
        let edited_segments = self.current_segments();
        let edited_for_commit = edited_segments.clone();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.toolbar.is_saving = true;
        cx.spawn(async move |shell, cx| {
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
            shell
                .update(cx, |shell, cx| {
                    shell.workbench.toolbar.is_saving = false;
                    if matches!(
                        result,
                        Ok(HistoryCommitTranscriptEditResult::Committed { .. }
                            | HistoryCommitTranscriptEditResult::Unchanged)
                    ) {
                        shell.workbench.base_segments = edited_segments;
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub fn begin_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<MainShell>) {
        self.editor.begin_edit(id, window, cx);
    }

    pub fn save_edit(&mut self, id: &str, cx: &mut Context<MainShell>) {
        self.editor.save_edit(id, cx);
        self.persist_current(cx);
    }

    pub fn persist_editor_change(&mut self, cx: &mut Context<MainShell>) {
        self.persist_current(cx);
    }

    pub fn delete_segment(&mut self, id: &str, cx: &mut Context<MainShell>) {
        self.editor.delete_segment(id);
        self.persist_current(cx);
    }

    pub fn merge_with_next(&mut self, id: &str, cx: &mut Context<MainShell>) {
        self.editor.merge_with_next(id);
        self.persist_current(cx);
    }

    pub fn split_segment(&mut self, id: &str, cx: &mut Context<MainShell>) {
        self.editor.split_segment(id);
        self.persist_current(cx);
    }

    pub fn set_load_error(&mut self) {
        self.is_loading = false;
        self.load_error = true;
        self.has_session = false;
    }

    pub fn open_rename(&mut self, window: &mut Window, cx: &mut Context<MainShell>) {
        let title = self.session_title.clone();
        self.rename_title_state.update(cx, |input, cx| {
            input.set_value(&title, window, cx);
        });
        self.is_rename_open = true;
        cx.notify();
    }

    pub fn save_rename(&mut self, cx: &mut Context<MainShell>) {
        let new_title = self.rename_title_state.read(cx).value().trim().to_string();
        if new_title.is_empty() {
            return;
        }
        self.session_title = new_title.clone();
        self.is_rename_open = false;
        if let Some(history_id) = self.history_id.clone() {
            let services = cx.global::<GlobalDesktopServices>().0.clone();
            cx.spawn(async move |shell, cx| {
                let _ = services
                    .history
                    .mutation_db(move |mutator| {
                        mutator.update_item_meta(HistoryUpdateItemMetaRequest {
                            history_id,
                            updates: HistoryItemMetaPatch {
                                title: Some(new_title),
                                ..Default::default()
                            },
                        })
                    })
                    .await;
                shell
                    .update(cx, |shell, cx| {
                        shell.project_selector.load_data(cx);
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
        }
        cx.notify();
    }

    pub fn generate_ai_title(&mut self, _window: &mut Window, cx: &mut Context<MainShell>) {
        let segments = self.current_segments();
        if segments.is_empty() {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.is_ai_renaming = true;
        cx.notify();

        cx.spawn(async move |shell, cx| {
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

            shell
                .update(cx, |shell, cx| {
                    if let Ok(title) = new_title {
                        shell.workbench.session_title = title;
                    }
                    shell.workbench.is_ai_renaming = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub fn start_polish(&mut self, cx: &mut Context<MainShell>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let segments = self.current_segments();
        if segments.is_empty() {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.is_polishing = true;
        self.polish_error = None;
        cx.notify();

        cx.spawn(async move |shell, cx| {
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

            shell
                .update(cx, |shell, cx| {
                    match result {
                        Ok(new_segments) => {
                            let duration = shell.workbench.player.total_duration;
                            shell.workbench.set_transcript(new_segments, duration);
                        }
                        Err(err) => {
                            shell.workbench.polish_error = Some(err);
                        }
                    }
                    shell.workbench.is_polishing = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub fn start_translate(&mut self, target_lang: &str, cx: &mut Context<MainShell>) {
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

        cx.spawn(async move |shell, cx| {
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

            shell
                .update(cx, |shell, cx| {
                    match result {
                        Ok(new_segments) => {
                            let duration = shell.workbench.player.total_duration;
                            shell.workbench.set_transcript(new_segments, duration);
                        }
                        Err(err) => {
                            shell.workbench.translate_error = Some(err);
                        }
                    }
                    shell.workbench.is_translating = false;
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
                    format!("{} ({})", snapshot.id, format!("{:?}", snapshot.reason)),
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

    pub fn save_summary_edit(&mut self, cx: &mut Context<MainShell>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let content = self.summary_sheet.editor_state.read(cx).value().to_string();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.summary_sheet.is_saving = true;
        self.summary_sheet.error_message = None;
        cx.spawn(async move |shell, cx| {
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
            shell
                .update(cx, |shell, cx| {
                    match result {
                        Ok(_) => shell.workbench.summary_sheet.summary_text = content,
                        Err(error) => shell.workbench.summary_sheet.error_message = Some(error),
                    }
                    shell.workbench.summary_sheet.is_saving = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub fn delete_summary(&mut self, cx: &mut Context<MainShell>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.summary_sheet.is_saving = true;
        self.summary_sheet.error_message = None;
        cx.spawn(async move |shell, cx| {
            let result = crate::platform::agent_control::facade::history::delete_summary(
                &services, history_id,
            )
            .await;
            shell
                .update(cx, |shell, cx| {
                    match result {
                        Ok(_) => shell.workbench.summary_sheet.summary_text.clear(),
                        Err(error) => shell.workbench.summary_sheet.error_message = Some(error),
                    }
                    shell.workbench.summary_sheet.is_saving = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub fn refresh_summary(&mut self, cx: &mut Context<MainShell>) {
        let Some(history_id) = self.history_id.clone() else {
            self.summary_sheet.is_generating = false;
            return;
        };
        let segments = self.current_segments();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.summary_sheet.is_generating = true;
        self.summary_sheet.error_message = None;
        cx.spawn(async move |shell, cx| {
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
            shell
                .update(cx, |shell, cx| {
                    match summary {
                        Ok(content) => {
                            shell.workbench.summary_sheet.summary_text = content;
                        }
                        Err(error) => {
                            shell.workbench.summary_sheet.error_message = Some(error);
                        }
                    }
                    shell.workbench.summary_sheet.is_generating = false;
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub fn restore_version(&mut self, snapshot_id: &str, cx: &mut Context<MainShell>) {
        let Some(history_id) = self.history_id.clone() else {
            return;
        };
        let snapshot_id = snapshot_id.to_string();
        let duration = self.player.total_duration;
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |shell, cx| {
            let result =
                crate::platform::agent_control::facade::transcript::revert_transcript_snapshot(
                    &services,
                    history_id.clone(),
                    snapshot_id,
                )
                .await;
            if result.is_ok() {
                if let Ok(Some(segments)) = services
                    .history
                    .query_db(move |query| query.load_transcript(&history_id))
                    .await
                {
                    shell
                        .update(cx, |shell, cx| {
                            shell.workbench.set_transcript(segments, duration);
                            shell.workbench.version_sheet.is_open = false;
                            cx.notify();
                        })
                        .ok();
                }
            }
        })
        .detach();
    }

    pub fn render(&mut self, window: &mut Window, cx: &mut Context<MainShell>) -> impl IntoElement {
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
                            .child(
                                Icon::new(LucideIcon::FileText)
                                    .size(px(16.0))
                                    .text_color(cx.theme().muted),
                            )
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.session_title.clone()),
                            )
                            .child(
                                Button::new("btn-rename-workbench")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Pencil)
                                    .tooltip("Rename")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.workbench.open_rename(window, cx);
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
                                        this.workbench.summary_sheet.toggle();
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
                                        this.workbench.speaker_sheet.toggle();
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
                                        this.workbench.version_sheet.toggle();
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
                                        this.workbench.start_polish(cx);
                                    })),
                            )
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
                                        this.workbench.start_translate("en", cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-export-workbench")
                                    .primary()
                                    .small()
                                    .icon(LucideIcon::Download)
                                    .label("Export")
                                    .tooltip("Export transcript to file")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let has_translation = this
                                            .workbench
                                            .editor
                                            .segments
                                            .iter()
                                            .any(|s| s.translation.is_some());
                                        this.export_modal.open(
                                            &this.workbench.session_title,
                                            has_translation,
                                            window,
                                            cx,
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-close-workbench")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::X)
                                    .tooltip("Close transcript")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workbench.has_session = false;
                                        this.workbench.history_id = None;
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
                            .top_4()
                            .right_4()
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
                            this.workbench.is_rename_open = false;
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
                                                    this.workbench.generate_ai_title(window, cx);
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
                                                    this.workbench.is_rename_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-save-rename")
                                                .primary()
                                                .small()
                                                .label("Save")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.workbench.save_rename(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            })
    }
}
