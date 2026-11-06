//! Create Workspace Panel coordinating Live Record and Batch Import sub-tabs.
//! Strict 1:1 parity with React `CreateWorkspacePanel.tsx` (Gap G06).

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::batch_import::BatchImportPanel;
use super::live_record::LiveRecordPanel;
use super::options_capsule::{IngestionSurface, TranscriptionOptionsModel};
use crate::theme::tokens::{COLOR_PROJECT_BLUE, RADIUS_FULL, RADIUS_MD, RADIUS_SM};

#[inline]
fn icon(name: LucideIcon) -> Icon {
    Icon::default().path(name.path())
}

/// Active sub-tab in Create Workspace.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum CreateWorkspaceTab {
    #[default]
    LiveRecord,
    BatchImport,
}

pub struct CreateWorkspacePanel {
    pub active_tab: CreateWorkspaceTab,
    pub live_record: Entity<LiveRecordPanel>,
    pub batch_import: Entity<BatchImportPanel>,
    pub options_model: Entity<TranscriptionOptionsModel>,
}

impl CreateWorkspacePanel {
    pub fn new(options_model: Entity<TranscriptionOptionsModel>, cx: &mut Context<Self>) -> Self {
        let opt_for_live = options_model.clone();
        let live_record = cx.new(|cx| {
            let mut panel = LiveRecordPanel::new(opt_for_live);
            panel.subscribe_audio_peaks(cx);
            panel
        });

        let opt_for_batch = options_model.clone();
        let batch_import = cx.new(|cx| {
            let mut panel = BatchImportPanel::new(opt_for_batch);
            panel.subscribe_batch_events(cx);
            panel
        });

        Self {
            active_tab: CreateWorkspaceTab::LiveRecord,
            live_record,
            batch_import,
            options_model,
        }
    }

    pub fn set_tab(&mut self, tab: CreateWorkspaceTab, cx: &mut Context<Self>) {
        if self.active_tab != tab {
            self.active_tab = tab;
            let target_surface = match tab {
                CreateWorkspaceTab::LiveRecord => IngestionSurface::Live,
                CreateWorkspaceTab::BatchImport => IngestionSurface::Batch,
            };
            self.options_model.update(cx, |opt, cx| {
                opt.set_surface(target_surface, cx);
            });
            cx.notify();
        }
    }

    pub fn toggle_tab(&mut self, cx: &mut Context<Self>) {
        let next_tab = match self.active_tab {
            CreateWorkspaceTab::LiveRecord => CreateWorkspaceTab::BatchImport,
            CreateWorkspaceTab::BatchImport => CreateWorkspaceTab::LiveRecord,
        };
        self.set_tab(next_tab, cx);
    }
}

impl Render for CreateWorkspacePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_live_active = self.active_tab == CreateWorkspaceTab::LiveRecord;
        let is_batch_active = self.active_tab == CreateWorkspaceTab::BatchImport;

        // Background activity indicators
        let show_live_recording_badge = self.live_record.read(cx).is_recording && !is_live_active;
        let is_batch_processing = self.batch_import.read(cx).is_processing();
        let show_batch_processing_badge = is_batch_processing && !is_batch_active;
        let batch_item_count = self.batch_import.read(cx).queue.len();
        let show_batch_count_badge =
            batch_item_count > 0 && !is_batch_processing && !is_batch_active;

        div()
            .id("create-workspace-panel")
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(
                // 54px Header with centered segmented control
                div()
                    .id("create-workspace-header")
                    .h(px(54.0))
                    .min_h(px(54.0))
                    .w_full()
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().background)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        // Segmented Navigation Track (p: 4px, rounded-md, bg: secondary, gap: 4px)
                        div()
                            .id("create-tab-navigation")
                            .flex()
                            .w_full()
                            .p(px(4.0))
                            .rounded(RADIUS_MD)
                            .bg(cx.theme().secondary.opacity(0.5))
                            .gap(px(4.0))
                            // Tab 1: Live Record
                            .child(
                                div()
                                    .id("create-tab-live")
                                    .flex_1()
                                    .h(px(32.0))
                                    .px(px(12.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.0))
                                    .rounded(RADIUS_SM)
                                    .cursor_pointer()
                                    .bg(if is_live_active {
                                        cx.theme().background
                                    } else {
                                        gpui_kit::transparent_black()
                                    })
                                    .shadow_sm()
                                    .text_size(px(13.5))
                                    .font_weight(if is_live_active {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_live_active {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(icon(LucideIcon::Mic).size(px(16.0)))
                                    .child(div().child("Live Record"))
                                    .when(show_live_recording_badge, |this| {
                                        // 7x7px Danger red pulsing dot
                                        this.child(
                                            div()
                                                .size(px(7.0))
                                                .rounded_full()
                                                .bg(cx.theme().danger),
                                        )
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_tab(CreateWorkspaceTab::LiveRecord, cx);
                                    })),
                            )
                            // Tab 2: Batch Import
                            .child(
                                div()
                                    .id("create-tab-batch")
                                    .flex_1()
                                    .h(px(32.0))
                                    .px(px(12.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.0))
                                    .rounded(RADIUS_SM)
                                    .cursor_pointer()
                                    .bg(if is_batch_active {
                                        cx.theme().background
                                    } else {
                                        gpui_kit::transparent_black()
                                    })
                                    .shadow_sm()
                                    .text_size(px(13.5))
                                    .font_weight(if is_batch_active {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_batch_active {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(icon(LucideIcon::Folder).size(px(16.0)))
                                    .child(div().child("Batch Import"))
                                    .when(show_batch_processing_badge, |this| {
                                        // 7x7px Blue pulsing processing dot
                                        this.child(
                                            div()
                                                .size(px(7.0))
                                                .rounded_full()
                                                .bg(COLOR_PROJECT_BLUE),
                                        )
                                    })
                                    .when(show_batch_count_badge, |this| {
                                        // 15px height count pill
                                        this.child(
                                            div()
                                                .h(px(15.0))
                                                .min_w(px(15.0))
                                                .px(px(4.0))
                                                .rounded(RADIUS_FULL)
                                                .bg(COLOR_PROJECT_BLUE)
                                                .text_color(gpui_kit::white())
                                                .text_size(px(10.0))
                                                .font_weight(FontWeight::BOLD)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(format!("{batch_item_count}")),
                                        )
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_tab(CreateWorkspaceTab::BatchImport, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                // Body (retains both panels in memory)
                div()
                    .id("create-workspace-body")
                    .flex_1()
                    .overflow_hidden()
                    .child(match self.active_tab {
                        CreateWorkspaceTab::LiveRecord => {
                            self.live_record.clone().into_any_element()
                        }
                        CreateWorkspaceTab::BatchImport => {
                            self.batch_import.clone().into_any_element()
                        }
                    }),
            )
    }
}
