use sona_core::transcription::streaming_stabilizer::{
    StreamingStabilizer, calculate_longest_common_prefix,
};

#[test]
fn test_calculate_longest_common_prefix_word_boundary() {
    let lcp = calculate_longest_common_prefix("I like to", "I like to read books");
    assert_eq!(lcp, "I like to");

    let lcp_cjk = calculate_longest_common_prefix("我喜欢", "我喜欢读书");
    assert_eq!(lcp_cjk, "我喜欢");

    let lcp_mid_token = calculate_longest_common_prefix("Hello wor", "Hello world");
    assert_eq!(lcp_mid_token, "Hello ");

    let lcp_empty = calculate_longest_common_prefix("", "Hello");
    assert_eq!(lcp_empty, "");
}

#[test]
fn test_streaming_stabilizer_local_agreement_and_finalization() {
    let mut stabilizer = StreamingStabilizer::new();

    // First hypothesis: no prior hypothesis, draft tail is whole hypothesis
    let out1 = stabilizer.update("I like to", false);
    assert_eq!(out1.frozen_prefix, None);
    assert_eq!(out1.draft_tail, Some("I like to".to_string()));

    // Second hypothesis: "I like to read books"
    // LCP of ("I like to", "I like to read books") is "I like to"
    let out2 = stabilizer.update("I like to read books", false);
    assert_eq!(out2.frozen_prefix, Some("I like to".to_string()));
    assert_eq!(out2.draft_tail, Some("read books".to_string()));

    // Third hypothesis extending further: "I like to read books every day"
    let out3 = stabilizer.update("I like to read books every day", false);
    assert_eq!(out3.frozen_prefix, Some("I like to read books".to_string()));
    assert_eq!(out3.draft_tail, Some("every day".to_string()));
    // Final hypothesis: is_final = true
    let out_final = stabilizer.update("I like to read books every day.", true);
    assert_eq!(
        out_final.frozen_prefix,
        Some("I like to read books every day.".to_string())
    );
    assert_eq!(out_final.draft_tail, None);
}

#[test]
fn test_streaming_stabilizer_cjk_progressive_stabilization() {
    let mut stabilizer = StreamingStabilizer::new();

    let out1 = stabilizer.update("我喜欢", false);
    assert_eq!(out1.frozen_prefix, None);
    assert_eq!(out1.draft_tail, Some("我喜欢".to_string()));

    let out2 = stabilizer.update("我喜欢看书", false);
    assert_eq!(out2.frozen_prefix, Some("我喜欢".to_string()));
    assert_eq!(out2.draft_tail, Some("看书".to_string()));

    let out_final = stabilizer.update("我喜欢看书。", true);
    assert_eq!(out_final.frozen_prefix, Some("我喜欢看书。".to_string()));
    assert_eq!(out_final.draft_tail, None);
}

#[test]
fn test_calculate_longest_common_prefix_symmetry_and_mid_word() {
    // Mid-word mismatch: "playing" vs "play time"
    let lcp1 = calculate_longest_common_prefix("playing", "play time");
    assert_eq!(lcp1, "");
    let lcp2 = calculate_longest_common_prefix("play time", "playing");
    assert_eq!(lcp2, "");

    // Boundary symmetry
    let lcp3 = calculate_longest_common_prefix("I like to read", "I like to");
    assert_eq!(lcp3, "I like to");
    let lcp4 = calculate_longest_common_prefix("I like to", "I like to read");
    assert_eq!(lcp4, "I like to");
}

#[test]
fn test_streaming_stabilizer_hypothesis_contraction_no_duplication() {
    let mut stabilizer = StreamingStabilizer::new();

    stabilizer.update("I like to read books", false);
    let out = stabilizer.update("I like to read books more", false);
    assert_eq!(out.frozen_prefix, Some("I like to read books".to_string()));
    assert_eq!(out.draft_tail, Some("more".to_string()));

    // Hypothesis contracts due to stream rollback/correction
    let out_contracted = stabilizer.update("I like to read", false);
    assert_eq!(
        out_contracted.frozen_prefix,
        Some("I like to read".to_string())
    );
    assert_eq!(out_contracted.draft_tail, None);

    // Hypothesis contracts further
    let out_smaller = stabilizer.update("I like", false);
    assert_eq!(out_smaller.frozen_prefix, Some("I like".to_string()));
    assert_eq!(out_smaller.draft_tail, None);
}
