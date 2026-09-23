use sona_core::llm::runtime::LlmStreamDeltaKind;

#[derive(Clone, Debug, Default)]
pub struct ThoughtStreamDemuxer {
    inside_think: bool,
    buffer: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemuxedChunk {
    pub kind: LlmStreamDeltaKind,
    pub text: String,
}

const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";

impl ThoughtStreamDemuxer {
    pub fn new() -> Self {
        Self {
            inside_think: false,
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
            if !self.inside_think {
                if let Some(idx) = remaining.find(THINK_OPEN) {
                    if idx > 0 {
                        chunks.push(DemuxedChunk {
                            kind: LlmStreamDeltaKind::Content,
                            text: remaining[..idx].to_string(),
                        });
                    }
                    self.inside_think = true;
                    remaining = &remaining[idx + THINK_OPEN.len()..];
                } else {
                    // Check if remaining ends with a partial prefix of THINK_OPEN (<think>)
                    let mut matched_prefix_len = 0;
                    for prefix_len in (1..THINK_OPEN.len()).rev() {
                        if remaining.ends_with(&THINK_OPEN[..prefix_len]) {
                            matched_prefix_len = prefix_len;
                            break;
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
            } else if let Some(idx) = remaining.find(THINK_CLOSE) {
                if idx > 0 {
                    chunks.push(DemuxedChunk {
                        kind: LlmStreamDeltaKind::Thought,
                        text: remaining[..idx].to_string(),
                    });
                }
                self.inside_think = false;
                remaining = &remaining[idx + THINK_CLOSE.len()..];
            } else {
                // Check if remaining ends with a partial prefix of THINK_CLOSE (</think>)
                let mut matched_prefix_len = 0;
                for prefix_len in (1..THINK_CLOSE.len()).rev() {
                    if remaining.ends_with(&THINK_CLOSE[..prefix_len]) {
                        matched_prefix_len = prefix_len;
                        break;
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
