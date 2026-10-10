use crate::client::IpcClient;
use crate::protocol::{ContentItem, PromptArgument, PromptDefinition, PromptMessage};

pub fn list_prompts() -> Vec<PromptDefinition> {
    vec![
        PromptDefinition {
            name: "summarize_meeting",
            title: Some("Summarize Meeting"),
            description: "Summarize a meeting transcript into key decisions, discussion highlights, and action items",
            arguments: vec![PromptArgument {
                name: "history_id",
                title: Some("History ID"),
                description: "History record ID to summarize",
                required: true,
            }],
        },
        PromptDefinition {
            name: "proofread_transcript",
            title: Some("Proofread Transcript"),
            description: "Proofread transcript segments for homophones, typos, and proper noun corrections",
            arguments: vec![PromptArgument {
                name: "history_id",
                title: Some("History ID"),
                description: "History record ID to proofread",
                required: true,
            }],
        },
    ]
}

pub async fn get_prompt(
    name: &str,
    arguments: serde_json::Value,
    client: &IpcClient,
) -> Result<Vec<PromptMessage>, String> {
    let history_id = arguments
        .get("history_id")
        .or_else(|| arguments.get("historyId"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required prompt argument 'history_id'".to_string())?;

    let transcript_res = client
        .call(
            "sona_read_transcript",
            serde_json::json!({ "history_id": history_id }),
        )
        .await?;

    match name {
        "summarize_meeting" => {
            let full_text = transcript_res
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim();

            let prompt_text = format!(
                "Please analyze and summarize the following meeting transcript for history record '{history_id}'.\n\n\
                ## Requirements\n\
                1. Core Decisions: Key agreements, decisions, or resolutions reached.\n\
                2. Discussion Highlights: Major topics discussed, viewpoints, and context.\n\
                3. Action Items: Clear list of tasks, owners, and expected deliverables.\n\n\
                ## Meeting Transcript\n\
                {full_text}"
            );

            Ok(vec![PromptMessage {
                role: "user",
                content: ContentItem {
                    item_type: "text",
                    text: prompt_text,
                },
            }])
        }
        "proofread_transcript" => {
            let segments = transcript_res
                .get("segments")
                .and_then(|s| serde_json::to_string_pretty(s).ok())
                .unwrap_or_else(|| "[]".to_string());

            let prompt_text = format!(
                "Please proofread the transcript segments for history record '{history_id}'.\n\n\
                ## Instructions\n\
                1. Identify phonetic recognition errors (homophones), grammatical typos, and misrecognized proper nouns or technical terms.\n\
                2. Preserve original timestamps (`start` and `end`) and segment IDs.\n\
                3. Propose exact corrections. To save these corrections back to the Sona client, use the `sona_edit_transcript` tool with the updated segments array.\n\n\
                ## Segments Data\n\
                {segments}"
            );

            Ok(vec![PromptMessage {
                role: "user",
                content: ContentItem {
                    item_type: "text",
                    text: prompt_text,
                },
            }])
        }
        _ => Err(format!("Prompt '{name}' not found")),
    }
}
