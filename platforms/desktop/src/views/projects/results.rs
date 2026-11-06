//! Projects results grid & cards list for transcript history.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

#[derive(Clone, Debug)]
pub struct HistoryRecordItem {
    pub id: String,
    pub title: String,
    pub duration: String,
    pub date: String,
    pub excerpt: String,
    pub speaker_count: usize,
}

pub struct ProjectsResults {
    pub items: Vec<HistoryRecordItem>,
    pub selected_id: Option<String>,
}

impl ProjectsResults {
    pub fn new() -> Self {
        Self {
            items: vec![
                HistoryRecordItem {
                    id: "item-1".to_string(),
                    title: "Quarterly Engineering Sync".to_string(),
                    duration: "42:15".to_string(),
                    date: "Today, 14:30".to_string(),
                    excerpt: "Discussing the pure Rust GPUI architecture migration and performance benchmarks...".to_string(),
                    speaker_count: 3,
                },
                HistoryRecordItem {
                    id: "item-2".to_string(),
                    title: "Audio Signal Processing Lecture".to_string(),
                    duration: "1:15:20".to_string(),
                    date: "Yesterday".to_string(),
                    excerpt: "Explaining Fourier transforms, windowing, and filterbank features for neural ASR...".to_string(),
                    speaker_count: 1,
                },
                HistoryRecordItem {
                    id: "item-3".to_string(),
                    title: "Product Roadmap Interview".to_string(),
                    duration: "28:40".to_string(),
                    date: "Nov 3, 2026".to_string(),
                    excerpt: "User experience requirements for transparent floating bilingual subtitle windows...".to_string(),
                    speaker_count: 2,
                },
            ],
            selected_id: None,
        }
    }

    pub fn select_item(&mut self, id: String) {
        self.selected_id = Some(id);
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .children(self.items.iter().map(|item| {
                let is_selected = self.selected_id.as_deref() == Some(&item.id);

                div()
                    .id(ElementId::from(format!("history-card-{}", item.id)))
                    .flex()
                    .flex_col()
                    .p_4()
                    .rounded_lg()
                    .bg(if is_selected {
                        cx.theme().accent
                    } else {
                        cx.theme().background
                    })
                    .border_1()
                    .border_color(if is_selected {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .shadow_sm()
                    .cursor_pointer()
                    .gap_2()
                    .child(
                        // Card header: Title + Duration
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(item.title.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().muted)
                                            .child(item.duration.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().muted)
                                            .child(format!("{} speakers", item.speaker_count)),
                                    ),
                            ),
                    )
                    .child(
                        // Excerpt
                        div()
                            .text_size(px(13.0))
                            .text_color(cx.theme().muted)
                            .child(item.excerpt.clone()),
                    )
                    .child(
                        // Footer: Date
                        div()
                            .text_size(px(11.0))
                            .text_color(cx.theme().muted)
                            .child(item.date.clone()),
                    )
            }))
    }
}
