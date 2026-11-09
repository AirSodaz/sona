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
use crate::theme::tokens::{
    COLOR_BG_ELEVATED_LIGHT, COLOR_BG_SECONDARY_LIGHT, COLOR_BG_TERTIARY_LIGHT,
    COLOR_TEXT_MUTED_LIGHT, COLOR_TEXT_PRIMARY_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_FULL,
    RADIUS_SM,
};
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
    live_tab_focus: FocusHandle,
    batch_tab_focus: FocusHandle,
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
            live_tab_focus: cx.focus_handle(),
            batch_tab_focus: cx.focus_handle(),
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_live_active = self.active_tab == CreateWorkspaceTab::LiveRecord;
        let is_batch_active = self.active_tab == CreateWorkspaceTab::BatchImport;

        // Background activity indicators
        let show_live_recording_badge = self.live_record.read(cx).is_recording && !is_live_active;
        let is_batch_processing = self.batch_import.read(cx).is_processing();
        let show_batch_processing_badge = is_batch_processing && !is_batch_active;
        let batch_item_count = self.batch_import.read(cx).queue.len();
        let show_batch_count_badge =
            batch_item_count > 0 && !is_batch_processing && !is_batch_active;
        let slider_progress = gpui_kit::base::motion::transition(
            "create-workspace-tab-slider",
            if is_batch_active { 1.0_f32 } else { 0.0_f32 },
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(220)).easing(
                gpui_kit::base::motion::Easing::CubicBezier {
                    x1: 0.4,
                    y1: 0.0,
                    x2: 0.2,
                    y2: 1.0,
                },
            ),
            window,
            cx,
        );

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
                    .bg(COLOR_BG_SECONDARY_LIGHT)
                    .child(
                        // Segmented Navigation Track (p: 4px, rounded-md, bg: tertiary, gap: 4px)
                        div()
                            .id("create-tab-navigation")
                            .flex()
                            .w_full()
                            .p(px(4.0))
                            .rounded(px(12.0))
                            .bg(COLOR_BG_TERTIARY_LIGHT)
                            .gap(px(4.0))
                            .role(Role::TabList)
                            .aria_label("Workspace Input Selection")
                            .aria_orientation(Orientation::Horizontal)
                            .child(
                                div()
                                    .id("create-tab-slider-track")
                                    .absolute()
                                    .top(px(4.0))
                                    .left(px(4.0))
                                    .right(px(8.0))
                                    .h(px(32.0))
                                    .child(
                                        div()
                                            .id("create-tab-slider")
                                            .absolute()
                                            .top(px(0.0))
                                            .left(relative(slider_progress * 0.5))
                                            .ml(px(slider_progress * 4.0))
                                            .w(relative(0.5))
                                            .h_full()
                                            .rounded(RADIUS_SM)
                                            .bg(COLOR_BG_ELEVATED_LIGHT)
                                            .shadow_sm(),
                                    ),
                            )
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                if event.keystroke.modifiers.alt
                                    || event.keystroke.modifiers.control
                                    || event.keystroke.modifiers.platform
                                {
                                    return;
                                }

                                let key = event.keystroke.key.to_ascii_lowercase();
                                let next_tab = match key.as_str() {
                                    "arrowright" | "right" | "arrowleft" | "left" => {
                                        Some(match this.active_tab {
                                            CreateWorkspaceTab::LiveRecord => {
                                                CreateWorkspaceTab::BatchImport
                                            }
                                            CreateWorkspaceTab::BatchImport => {
                                                CreateWorkspaceTab::LiveRecord
                                            }
                                        })
                                    }
                                    "arrowdown" | "down" | "end" => {
                                        Some(CreateWorkspaceTab::BatchImport)
                                    }
                                    "arrowup" | "up" | "home" => {
                                        Some(CreateWorkspaceTab::LiveRecord)
                                    }
                                    _ => None,
                                };
                                if let Some(tab) = next_tab {
                                    window.prevent_default();
                                    this.set_tab(tab, cx);
                                    let focus_handle = match tab {
                                        CreateWorkspaceTab::LiveRecord => &this.live_tab_focus,
                                        CreateWorkspaceTab::BatchImport => &this.batch_tab_focus,
                                    };
                                    window.focus(focus_handle, cx);
                                }
                            }))
                            // Tab 1: Live Record
                            .child(
                                div()
                                    .id("create-tab-live")
                                    .track_focus(&self.live_tab_focus)
                                    .role(Role::Tab)
                                    .aria_label("Live Record")
                                    .aria_selected(is_live_active)
                                    .flex_1()
                                    .tab_index(if is_live_active { 0 } else { -1 })
                                    .h(px(32.0))
                                    .px(px(12.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.0))
                                    .rounded(RADIUS_SM)
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(cx.theme().primary)
                                    })
                                    .text_size(px(14.0))
                                    .font_weight(if is_live_active {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_live_active {
                                        COLOR_TEXT_PRIMARY_LIGHT
                                    } else {
                                        COLOR_TEXT_MUTED_LIGHT
                                    })
                                    .when(!is_live_active, |this| {
                                        this.hover(|style| {
                                            style.text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                        })
                                    })
                                    .child(icon(LucideIcon::Mic).size(px(16.0)))
                                    .child(div().child("Live Record"))
                                    .when(show_live_recording_badge, |this| {
                                        // 7x7px Danger red pulsing dot
                                        this.child(
                                            div()
                                                .size(px(7.0))
                                                .rounded_full()
                                                .bg(hsla(0.0, 0.84, 0.60, 1.0)),
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
                                    .track_focus(&self.batch_tab_focus)
                                    .role(Role::Tab)
                                    .aria_label("Batch Import")
                                    .aria_selected(is_batch_active)
                                    .flex_1()
                                    .tab_index(if is_batch_active { 0 } else { -1 })
                                    .h(px(32.0))
                                    .px(px(12.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.0))
                                    .rounded(RADIUS_SM)
                                    .cursor_pointer()
                                    .focus_visible(|style| {
                                        style.border_1().border_color(cx.theme().primary)
                                    })
                                    .text_size(px(14.0))
                                    .font_weight(if is_batch_active {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_batch_active {
                                        COLOR_TEXT_PRIMARY_LIGHT
                                    } else {
                                        COLOR_TEXT_MUTED_LIGHT
                                    })
                                    .when(!is_batch_active, |this| {
                                        this.hover(|style| {
                                            style.text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                        })
                                    })
                                    .child(icon(LucideIcon::Folder).size(px(16.0)))
                                    .child(div().child("Batch Import"))
                                    .when(show_batch_processing_badge, |this| {
                                        // 7x7px Blue pulsing processing dot
                                        this.child(div().size(px(7.0)).rounded_full().bg(hsla(
                                            196.0 / 360.0,
                                            0.86,
                                            0.322,
                                            1.0,
                                        )))
                                    })
                                    .when(show_batch_count_badge, |this| {
                                        // 15px height count pill
                                        this.child(
                                            div()
                                                .h(px(15.0))
                                                .min_w(px(15.0))
                                                .px(px(3.5))
                                                .rounded(RADIUS_FULL)
                                                .bg(hsla(196.0 / 360.0, 0.86, 0.322, 1.0))
                                                .text_color(gpui_kit::white())
                                                .text_size(px(9.6))
                                                .font_weight(FontWeight::SEMIBOLD)
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
