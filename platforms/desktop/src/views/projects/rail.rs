//! Projects left sidebar rail.

use crate::theme::tokens::*;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectItem {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub color_index: usize,
}

pub struct ProjectsRail {
    pub selected_project: Option<String>,
    pub projects: Vec<ProjectItem>,
}

impl ProjectsRail {
    pub fn new() -> Self {
        Self {
            selected_project: None,
            projects: vec![
                ProjectItem {
                    id: "proj-1".to_string(),
                    name: "Work Meetings".to_string(),
                    count: 14,
                    color_index: 0,
                },
                ProjectItem {
                    id: "proj-2".to_string(),
                    name: "Lectures & Courses".to_string(),
                    count: 8,
                    color_index: 1,
                },
                ProjectItem {
                    id: "proj-3".to_string(),
                    name: "Interviews".to_string(),
                    count: 5,
                    color_index: 2,
                },
            ],
        }
    }

    pub fn select_project(&mut self, id: Option<String>) {
        self.selected_project = id;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        let is_all_selected = self.selected_project.is_none();

        div()
            .flex()
            .flex_col()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                // Section header
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted)
                            .child("WORKSPACES"),
                    )
                    .child(Button::new("btn-new-project").ghost().small().label("+")),
            )
            .child(
                // "All Items" button
                div()
                    .id(ElementId::from("rail-all-items"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if is_all_selected {
                        cx.theme().accent
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .cursor_pointer()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if is_all_selected {
                                cx.theme().primary
                            } else {
                                cx.theme().foreground
                            })
                            .child("All Transcripts"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted)
                            .child("27"),
                    ),
            )
            .child(
                // Divider
                div().my_2().h(px(1.0)).bg(cx.theme().border),
            )
            .child(
                // Projects list
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.projects.iter().map(|p| {
                        let is_selected = self.selected_project.as_deref() == Some(&p.id);
                        let color = project_color_from_index(p.color_index);

                        div()
                            .id(ElementId::from(format!("rail-proj-{}", p.id)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(if is_selected {
                                cx.theme().accent
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .cursor_pointer()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(color))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(p.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child(p.count.to_string()),
                            )
                    })),
            )
    }
}
