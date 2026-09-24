use sona_core::llm::runtime::LlmStreamDeltaKind;

#[derive(Clone, Debug, Default)]
pub struct ThoughtStreamDemuxer {
    inside_think: bool,
    active_close_tag: Option<&'static str>,
    buffer: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemuxedChunk {
    pub kind: LlmStreamDeltaKind,
    pub text: String,
}

const TAG_PAIRS: &[(&str, &str)] = &[
    ("<think>", "</think>"),
    ("<thought>", "</thought>"),
    ("<thinking>", "</thinking>"),
    ("<reasoning>", "</reasoning>"),
];

impl ThoughtStreamDemuxer {
    pub fn new() -> Self {
        Self {
            inside_think: false,
            active_close_tag: None,
            buffer: String::new(),
        }
    }

    pub fn process(&mut self, text: &str) -> Vec<DemuxedChunk> {
        let mut chunks = Vec::new();
        let full_text = if self.buffer.is_empty() {
            text.to_string()
        } else {
            let mut s = std::mem::take(&mut self.buffer);
            s.push_str(text);
            s
        };

        let mut remaining = full_text.as_str();

        while !remaining.is_empty() {
            let lower = remaining.to_lowercase();
            if !self.inside_think {
                // Find earliest opening tag
                let mut earliest: Option<(usize, &'static str, &'static str)> = None;
                for (open_tag, close_tag) in TAG_PAIRS {
                    if let Some(idx) = lower.find(open_tag) {
                        if let Some((best_idx, _, _)) = earliest {
                            if idx < best_idx {
                                earliest = Some((idx, open_tag, close_tag));
                            }
                        } else {
                            earliest = Some((idx, open_tag, close_tag));
                        }
                    }
                }

                if let Some((idx, open_tag, close_tag)) = earliest {
                    if idx > 0 {
                        chunks.push(DemuxedChunk {
                            kind: LlmStreamDeltaKind::Content,
                            text: remaining[..idx].to_string(),
                        });
                    }
                    self.inside_think = true;
                    self.active_close_tag = Some(close_tag);
                    remaining = &remaining[idx + open_tag.len()..];
                } else {
                    // Check if remaining ends with a partial prefix of any open tag
                    let mut matched_prefix_len = 0;
                    for (open_tag, _) in TAG_PAIRS {
                        for prefix_len in (1..open_tag.len()).rev() {
                            if lower.ends_with(&open_tag[..prefix_len]) {
                                matched_prefix_len = matched_prefix_len.max(prefix_len);
                                break;
                            }
                        }
                    }

                    if matched_prefix_len > 0 {
                        let safe_content_len = remaining.len() - matched_prefix_len;
                        if safe_content_len > 0 {
                            chunks.push(DemuxedChunk {
                                kind: LlmStreamDeltaKind::Content,
                                text: remaining[..safe_content_len].to_string(),
                            });
                        }
                        self.buffer = remaining[safe_content_len..].to_string();
                    } else {
                        chunks.push(DemuxedChunk {
                            kind: LlmStreamDeltaKind::Content,
                            text: remaining.to_string(),
                        });
                    }
                    break;
                }
            } else {
                // Inside thought: look for closing tag
                let mut earliest_close: Option<(usize, &'static str)> = None;
                if let Some(expected_close) = self.active_close_tag
                    && let Some(idx) = lower.find(expected_close)
                {
                    earliest_close = Some((idx, expected_close));
                }
                if earliest_close.is_none() {
                    for (_, close_tag) in TAG_PAIRS {
                        if let Some(idx) = lower.find(close_tag) {
                            if let Some((best_idx, _)) = earliest_close {
                                if idx < best_idx {
                                    earliest_close = Some((idx, close_tag));
                                }
                            } else {
                                earliest_close = Some((idx, close_tag));
                            }
                        }
                    }
                }

                if let Some((idx, close_tag)) = earliest_close {
                    if idx > 0 {
                        chunks.push(DemuxedChunk {
                            kind: LlmStreamDeltaKind::Thought,
                            text: remaining[..idx].to_string(),
                        });
                    }
                    self.inside_think = false;
                    self.active_close_tag = None;
                    let mut after_close = idx + close_tag.len();
                    // Strip optional immediate newline after close tag
                    if remaining[after_close..].starts_with("\r\n") {
                        after_close += 2;
                    } else if remaining[after_close..].starts_with('\n') {
                        after_close += 1;
                    }
                    remaining = &remaining[after_close..];
                } else {
                    // Check if remaining ends with a partial prefix of closing tag
                    let mut matched_prefix_len = 0;
                    for (_, close_tag) in TAG_PAIRS {
                        if let Some(expected) = self.active_close_tag
                            && *close_tag != expected
                        {
                            continue;
                        }
                        for prefix_len in (1..close_tag.len()).rev() {
                            if lower.ends_with(&close_tag[..prefix_len]) {
                                matched_prefix_len = matched_prefix_len.max(prefix_len);
                                break;
                            }
                        }
                    }
                    if matched_prefix_len > 0 {
                        let safe_thought_len = remaining.len() - matched_prefix_len;
                        if safe_thought_len > 0 {
                            chunks.push(DemuxedChunk {
                                kind: LlmStreamDeltaKind::Thought,
                                text: remaining[..safe_thought_len].to_string(),
                            });
                        }
                        self.buffer = remaining[safe_thought_len..].to_string();
                    } else {
                        chunks.push(DemuxedChunk {
                            kind: LlmStreamDeltaKind::Thought,
                            text: remaining.to_string(),
                        });
                    }
                    break;
                }
            }
        }

        chunks
    }

    pub fn flush(&mut self) -> Vec<DemuxedChunk> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let text = std::mem::take(&mut self.buffer);
        vec![DemuxedChunk {
            kind: if self.inside_think {
                LlmStreamDeltaKind::Thought
            } else {
                LlmStreamDeltaKind::Content
            },
            text,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processes_plain_content_without_tags() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        let chunks = demuxer.process("Hello world");
        assert_eq!(
            chunks,
            vec![DemuxedChunk {
                kind: LlmStreamDeltaKind::Content,
                text: "Hello world".to_string(),
            }]
        );
    }

    #[test]
    fn demuxes_inline_think_tags_within_single_chunk() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        let chunks = demuxer.process("<think>reasoning step</think>final answer");
        assert_eq!(
            chunks,
            vec![
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Thought,
                    text: "reasoning step".to_string(),
                },
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Content,
                    text: "final answer".to_string(),
                },
            ]
        );
    }

    #[test]
    fn demuxes_case_insensitive_and_alternative_tags() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        let chunks = demuxer.process("<Thought>deep reasoning</Thought>\nclean result");
        assert_eq!(
            chunks,
            vec![
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Thought,
                    text: "deep reasoning".to_string(),
                },
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Content,
                    text: "clean result".to_string(),
                },
            ]
        );

        let mut demuxer2 = ThoughtStreamDemuxer::new();
        let chunks2 = demuxer2.process("<THINKING>step 1</THINKING>response");
        assert_eq!(
            chunks2,
            vec![
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Thought,
                    text: "step 1".to_string(),
                },
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Content,
                    text: "response".to_string(),
                },
            ]
        );
    }

    #[test]
    fn demuxes_across_chunk_boundaries() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        let c1 = demuxer.process("Prefix <think>Step 1 ");
        let c2 = demuxer.process("Step 2</think> Answer");

        assert_eq!(
            c1,
            vec![
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Content,
                    text: "Prefix ".to_string(),
                },
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Thought,
                    text: "Step 1 ".to_string(),
                },
            ]
        );
        assert_eq!(
            c2,
            vec![
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Thought,
                    text: "Step 2".to_string(),
                },
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Content,
                    text: " Answer".to_string(),
                },
            ]
        );
    }

    #[test]
    fn demuxes_partial_opening_and_closing_tags_split_across_chunks() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        // Opening tag split: "Prefix <thi" + "nk>Step 1"
        let c1 = demuxer.process("Prefix <thi");
        assert_eq!(
            c1,
            vec![DemuxedChunk {
                kind: LlmStreamDeltaKind::Content,
                text: "Prefix ".to_string(),
            }]
        );

        let c2 = demuxer.process("nk>Step 1");
        assert_eq!(
            c2,
            vec![DemuxedChunk {
                kind: LlmStreamDeltaKind::Thought,
                text: "Step 1".to_string(),
            }]
        );

        // Closing tag split: "</thi" + "nk> Answer"
        let c3 = demuxer.process("</thi");
        assert_eq!(c3, vec![]);

        let c4 = demuxer.process("nk> Answer");
        assert_eq!(
            c4,
            vec![DemuxedChunk {
                kind: LlmStreamDeltaKind::Content,
                text: " Answer".to_string(),
            }]
        );
    }

    #[test]
    fn flush_emits_pending_non_tag_buffer() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        let c1 = demuxer.process("Hello <th");
        assert_eq!(
            c1,
            vec![DemuxedChunk {
                kind: LlmStreamDeltaKind::Content,
                text: "Hello ".to_string(),
            }]
        );
        let flushed = demuxer.flush();
        assert_eq!(
            flushed,
            vec![DemuxedChunk {
                kind: LlmStreamDeltaKind::Content,
                text: "<th".to_string(),
            }]
        );
    }
}
