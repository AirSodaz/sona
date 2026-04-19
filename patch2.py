import re

with open("src-tauri/src/llm.rs", "r") as f:
    content = f.read()

# Fix 1: build_polish_prompt and build_translate_prompt should take &[LlmSegmentInput]
# The error shows they currently take `mut segments: Vec<LlmSegmentInput>`
# Let's check what their signature is.
content = content.replace(
    "fn build_polish_prompt(\n    mut segments: Vec<LlmSegmentInput>,",
    "fn build_polish_prompt(\n    segments: &[LlmSegmentInput],"
)
content = content.replace(
    "fn build_translate_prompt(mut segments: Vec<LlmSegmentInput>, target_language: &str) -> String {",
    "fn build_translate_prompt(segments: &[LlmSegmentInput], target_language: &str) -> String {"
)
# We also had errors with serde_json::to_string(segments). Since we change it back to `&[LlmSegmentInput]`, it's naturally a reference!
# But let's make sure it is passed as a reference.

with open("src-tauri/src/llm.rs", "w") as f:
    f.write(content)
