pub use sona_core::llm::demuxer::*;
pub use sona_core::llm::runtime::LlmStreamDeltaKind;
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

    #[test]
    fn handles_unicode_length_changing_chars_without_panic() {
        let mut demuxer = ThoughtStreamDemuxer::new();
        // '\u{212A}' is Kelvin sign (3 bytes in UTF-8, 1 byte when lowercased)
        let text = "\u{212A}<think>reasoning step</think>final answer";
        let chunks = demuxer.process(text);
        assert_eq!(
            chunks,
            vec![
                DemuxedChunk {
                    kind: LlmStreamDeltaKind::Content,
                    text: "\u{212A}".to_string(),
                },
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
}
