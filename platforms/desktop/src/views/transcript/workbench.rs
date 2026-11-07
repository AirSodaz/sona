//! Transcript Workbench view combining Editor, Player, Toolbar, and Sheets.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;
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
use sona_core::history::mutation_repository::{
    HistoryCommitTranscriptEditRequest, HistoryCommitTranscriptEditResult,
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
}

impl TranscriptWorkbench {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
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
            let message = if self.is_loading {
                "Loading transcript…"
            } else if self.load_error {
                "Couldn’t load this transcript"
            } else {
                "Select a transcript to open it"
            };
            return div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(cx.theme().muted)
                .child(message);
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
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
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.session_title.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("btn-open-summary")
                                    .outline()
                                    .small()
                                    .label("AI Summary")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workbench.summary_sheet.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-open-speakers")
                                    .outline()
                                    .small()
                                    .label("Speakers")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workbench.speaker_sheet.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-open-versions")
                                    .outline()
                                    .small()
                                    .label("History")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workbench.version_sheet.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-export-workbench")
                                    .primary()
                                    .small()
                                    .label("Export")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_modal.open();
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
    }
}
