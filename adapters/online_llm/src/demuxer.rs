use sona_core::llm::runtime::LlmStreamDeltaKind;

#[derive(Clone, Debug, Default)]
pub struct ThoughtStreamDemuxer {
    inside_think: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemuxedChunk {
    pub kind: LlmStreamDeltaKind,
    pub text: String,
}

impl ThoughtStreamDemuxer {
    pub fn new() -> Self {
        Self {
            inside_think: false,
        }
    }

    pub fn process(&mut self, text: &str) -> Vec<DemuxedChunk> {
        let mut chunks = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if !self.inside_think {
                if let Some(idx) = remaining.find("<think>") {
                    if idx > 0 {
                        chunks.push(DemuxedChunk {
                            kind: LlmStreamDeltaKind::Content,
                            text: remaining[..idx].to_string(),
                        });
                    }
                    self.inside_think = true;
                    remaining = &remaining[idx + "<think>".len()..];
                } else {
                    chunks.push(DemuxedChunk {
                        kind: LlmStreamDeltaKind::Content,
                        text: remaining.to_string(),
                    });
                    break;
                }
            } else if let Some(idx) = remaining.find("</think>") {
                if idx > 0 {
                    chunks.push(DemuxedChunk {
                        kind: LlmStreamDeltaKind::Thought,
                        text: remaining[..idx].to_string(),
                    });
                }
                self.inside_think = false;
                remaining = &remaining[idx + "</think>".len()..];
            } else {
                chunks.push(DemuxedChunk {
                    kind: LlmStreamDeltaKind::Thought,
                    text: remaining.to_string(),
                });
                break;
            }
        }

        chunks
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
}
