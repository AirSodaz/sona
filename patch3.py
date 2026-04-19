import re

with open("src-tauri/src/llm.rs", "r") as f:
    content = f.read()

content = content.replace(
"""        let err = parse_polish_chunk(
            r#"[{"id":"1","text":"Hello"}]"#,
            &sample_segments()[..2],
            1,
        )""",
"""        let err = parse_polish_chunk(
            r#"[{"id":"1","text":"Hello"}]"#,
            sample_segments()[..2].to_vec(),
            1,
        )"""
)

content = content.replace(
"""        let err = parse_translate_chunk(
            r#"[{"id":"2","translation":"Bonjour"}]"#,
            &sample_segments()[..2],
            1,
        )""",
"""        let err = parse_translate_chunk(
            r#"[{"id":"2","translation":"Bonjour"}]"#,
            sample_segments()[..2].to_vec(),
            1,
        )"""
)


content = content.replace(
"""        let result = run_segment_task(
            "task-1",
            LlmTaskType::Polish,
            &segments,""",
"""        let result = run_segment_task(
            "task-1",
            LlmTaskType::Polish,
            segments,"""
)


content = content.replace(
"""        let err = run_segment_task(
            "task-2",
            LlmTaskType::Translate,
            &segments,""",
"""        let err = run_segment_task(
            "task-2",
            LlmTaskType::Translate,
            segments,"""
)


with open("src-tauri/src/llm.rs", "w") as f:
    f.write(content)
