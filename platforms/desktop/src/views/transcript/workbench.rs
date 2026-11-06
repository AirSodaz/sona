//! Transcript Workbench view combining Editor, Player, Toolbar, and Sheets.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

use super::audio_player::AudioPlayerBar;
use super::editor::TranscriptEditor;
use super::search_overlay::SearchOverlay;
use super::speaker_review_sheet::SpeakerReviewSheet;
use super::summary_sheet::SummarySheet;
use super::toolbar::EditorToolbar;
use super::version_sheet::VersionSheet;

pub struct TranscriptWorkbench {
    pub session_title: String,
    pub editor: TranscriptEditor,
    pub toolbar: EditorToolbar,
    pub player: AudioPlayerBar,
    pub search: SearchOverlay,
    pub summary_sheet: SummarySheet,
    pub speaker_sheet: SpeakerReviewSheet,
    pub version_sheet: VersionSheet,
}

impl TranscriptWorkbench {
    pub fn new() -> Self {
        Self {
            session_title: "Engineering Sync - GPUI Architecture.wav".to_string(),
            editor: TranscriptEditor::new(),
            toolbar: EditorToolbar::new(),
            player: AudioPlayerBar::new(),
            search: SearchOverlay::new(),
            summary_sheet: SummarySheet::new(),
            speaker_sheet: SpeakerReviewSheet::new(),
            version_sheet: VersionSheet::new(),
        }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
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
                                    .label("AI Summary"),
                            )
                            .child(
                                Button::new("btn-open-speakers")
                                    .outline()
                                    .small()
                                    .label("Speakers"),
                            )
                            .child(
                                Button::new("btn-open-versions")
                                    .outline()
                                    .small()
                                    .label("History"),
                            )
                            .child(
                                Button::new("btn-export-workbench")
                                    .primary()
                                    .small()
                                    .label("Export"),
                            ),
                    ),
            )
            .child(
                // Formatting toolbar
                self.toolbar.render(cx),
            )
            .child(
                // Center split area: Editor + optional drawers
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .child(div().flex().flex_1().h_full().child(self.editor.render(cx)))
                    .child(self.summary_sheet.render(cx))
                    .child(self.speaker_sheet.render(cx))
                    .child(self.version_sheet.render(cx)),
            )
            .child(
                // Sticky bottom audio player
                self.player.render(cx),
            )
    }
}
