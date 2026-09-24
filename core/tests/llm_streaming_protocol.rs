use sona_core::llm::runtime::LlmStreamDelta;
use sona_core::llm::streaming_protocol::{DualStreamAccumulator, StreamTextAccumulator};
use std::convert::Infallible;

#[test]
fn stream_text_accumulator_emits_full_text_and_delta() {
    let mut emitted = Vec::new();
    let mut emit_delta = |text: &str, delta: &str| {
        emitted.push((text.to_string(), delta.to_string()));
        Ok::<(), Infallible>(())
    };
    let mut accumulator = StreamTextAccumulator::new(&mut emit_delta);

    accumulator.push("").expect("empty delta should be ignored");
    accumulator.push("Hel").expect("first delta should emit");
    accumulator.push("lo").expect("second delta should emit");

    drop(accumulator);
    assert_eq!(
        emitted,
        vec![
            ("Hel".to_string(), "Hel".to_string()),
            ("Hello".to_string(), "lo".to_string()),
        ]
    );
}

#[derive(Debug, PartialEq, Eq)]
struct EmitFailure(&'static str);

#[test]
fn stream_text_accumulator_preserves_callback_error_type() {
    let mut emit_delta = |_text: &str, _delta: &str| Err(EmitFailure("observer closed"));
    let mut accumulator = StreamTextAccumulator::new(&mut emit_delta);

    assert_eq!(
        accumulator.push("hello").unwrap_err(),
        EmitFailure("observer closed")
    );
}

#[test]
fn dual_stream_accumulator_separates_thought_and_content() {
    let mut events = Vec::new();
    let mut emit = |delta: LlmStreamDelta| {
        events.push(delta);
        Ok::<(), ()>(())
    };
    let mut accumulator = DualStreamAccumulator::new(&mut emit);

    accumulator.push_thought("Thinking step 1...").unwrap();
    accumulator.push_thought("Thinking step 2...").unwrap();
    accumulator.push_content("Final ").unwrap();
    accumulator.push_content("Answer").unwrap();

    assert_eq!(
        accumulator.thought_text(),
        "Thinking step 1...Thinking step 2..."
    );
    assert_eq!(accumulator.content_text(), "Final Answer");
    assert_eq!(events.len(), 4);
    assert!(events[0].is_thought());
    assert_eq!(events[0].delta, "Thinking step 1...");
    assert!(events[1].is_thought());
    assert_eq!(events[1].delta, "Thinking step 2...");
    assert!(!events[2].is_thought());
    assert_eq!(events[2].delta, "Final ");
    assert!(!events[3].is_thought());
    assert_eq!(events[3].delta, "Answer");
}
