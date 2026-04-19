import re

with open("src-tauri/src/llm.rs", "r") as f:
    content = f.read()

content = content.replace(
"""        let err = parse_translate_chunk(
            r#"[{"id":"2","translation":"B"},{"id":"1","translation":"A"}]"#,
            &sample_segments()[..2],
            2,
        )""",
"""        let err = parse_translate_chunk(
            r#"[{"id":"2","translation":"B"},{"id":"1","translation":"A"}]"#,
            sample_segments()[..2].to_vec(),
            2,
        )"""
)

with open("src-tauri/src/llm.rs", "w") as f:
    f.write(content)
