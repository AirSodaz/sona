use serde_json::Value;
use sona_core::config::{AppConfig, TextReplacementSetRecord};
use sona_core::export::{ExportFormat, ExportMode, ExportTranscriptFileRequest};
use sona_core::history::mutation_repository::HistoryUpdateTranscriptRequest;
use sona_core::project::EffectivePipelineSnapshot;
use sona_core::transcription::postprocess::{
    TranscriptPostprocessOptions, TranscriptPostprocessor, TranscriptTextReplacementRule,
    TranscriptTextReplacementRuleSet,
};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_core::{config::TextReplacementRuleRecord, llm::tasks::SummaryTemplateConfig};
use std::path::Path;

use crate::services::DesktopServices;

pub struct BatchPipelineResult {
    pub segments: Vec<TranscriptSegment>,
    pub export_path: Option<String>,
}

pub struct BatchPipelineOptions<'a> {
    pub config: &'a Value,
    pub pipeline: &'a EffectivePipelineSnapshot,
    pub project_id: Option<&'a str>,
    pub history_id: &'a str,
    pub filename: &'a str,
    pub segments: Vec<TranscriptSegment>,
    pub cancelled: &'a dyn Fn() -> bool,
}

pub async fn execute(
    services: &DesktopServices,
    options: BatchPipelineOptions<'_>,
) -> Result<BatchPipelineResult, String> {
    let BatchPipelineOptions {
        config,
        pipeline,
        project_id,
        history_id,
        filename,
        segments,
        cancelled,
    } = options;
    let mut current = segments;
    if current.is_empty() {
        return Ok(BatchPipelineResult {
            segments: current,
            export_path: None,
        });
    }
    ensure_active(&cancelled)?;

    let replacements = replacement_sets(config, pipeline, project_id)?;
    if !replacements.is_empty() {
        let processor = TranscriptPostprocessor::compile(TranscriptPostprocessOptions {
            text_replacement_sets: replacements,
            drop_final_dot_segments: false,
        })
        .map_err(|error| error.to_string())?;
        current = processor.process_segments(current);
        persist_segments(services, history_id, current.clone()).await?;
    }

    let data_dir = crate::platform::storage_location::default_app_local_data_dir();
    let models_dir =
        crate::platform::storage_location::resolve_active_models_dir_for_path(&data_dir);
    let adapter = crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);

    if pipeline.auto_polish {
        ensure_active(&cancelled)?;
        if let Some(llm_config) =
            crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                config, "polish",
            )
        {
            let inputs = current
                .iter()
                .map(|segment| sona_core::llm::tasks::LlmSegmentInput {
                    id: segment.id.clone(),
                    text: segment.text.clone(),
                })
                .collect();
            let context = polish_context(config, pipeline);
            let preset_id = pipeline
                .polish_preset_id
                .as_deref()
                .or_else(|| config.get("polishPresetId").and_then(Value::as_str))
                .unwrap_or("clean");
            let mode = match preset_id {
                "verbatim" => sona_core::llm::tasks::PolishMode::Verbatim,
                "formal" => sona_core::llm::tasks::PolishMode::Formal,
                _ => sona_core::llm::tasks::PolishMode::Clean,
            };
            let request = sona_core::llm::requests::PolishSegmentsRequest {
                task_id: format!("batch-polish-{}", uuid::Uuid::new_v4()),
                config: llm_config,
                segments: inputs,
                chunk_size: None,
                context,
                keywords: config
                    .get("polishKeywords")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                mode: Some(mode),
            };
            let polished = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                sona_application::llm::LlmTaskService::new(adapter.clone()).polish(request, &()),
            )
            .await
            .map_err(|_| "Polish generation timed out".to_string())?
            .map_err(|error| error.to_string())?;
            for result in polished {
                if let Some(segment) = current.iter_mut().find(|segment| segment.id == result.id) {
                    segment.text = result.text;
                }
            }
            persist_segments(services, history_id, current.clone()).await?;
        }
    }

    if pipeline.auto_translate {
        ensure_active(&cancelled)?;
        if let Some(llm_config) =
            crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                config,
                "translation",
            )
        {
            let target_language = pipeline
                .target_language
                .as_deref()
                .or_else(|| config.get("translationLanguage").and_then(Value::as_str))
                .unwrap_or("zh-CN")
                .to_string();
            let inputs = current
                .iter()
                .filter(|segment| {
                    !segment.text.trim().is_empty()
                        && (pipeline.auto_polish
                            || segment
                                .translation
                                .as_deref()
                                .is_none_or(|value| value.trim().is_empty()))
                })
                .map(|segment| sona_core::llm::tasks::LlmSegmentInput {
                    id: segment.id.clone(),
                    text: segment.text.clone(),
                })
                .collect::<Vec<_>>();
            if !inputs.is_empty() {
                let request = sona_core::llm::requests::TranslateSegmentsRequest {
                    task_id: format!("batch-translate-{}", uuid::Uuid::new_v4()),
                    config: llm_config,
                    segments: inputs,
                    chunk_size: None,
                    target_language,
                    target_language_name: None,
                    source_language: None,
                    context: None,
                    keywords: None,
                };
                let translated = tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    sona_application::llm::LlmTaskService::new(adapter.clone())
                        .translate(request, &()),
                )
                .await
                .map_err(|_| "Translation timed out".to_string())?
                .map_err(|error| error.to_string())?;
                for result in translated {
                    if let Some(segment) =
                        current.iter_mut().find(|segment| segment.id == result.id)
                    {
                        segment.translation =
                            Some(crate::services::text_format::format_cjk_typography(
                                &result.translation,
                            ));
                    }
                }
                persist_segments(services, history_id, current.clone()).await?;
            }
        }
    }

    if pipeline.auto_summary {
        ensure_active(&cancelled)?;
        if let Some(llm_config) =
            crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
                config, "summary",
            )
        {
            let template_id = pipeline
                .summary_template_id
                .as_deref()
                .or_else(|| config.get("summaryTemplateId").and_then(Value::as_str))
                .unwrap_or("general");
            let template = summary_template(config, template_id);
            let request = sona_core::llm::requests::SummarizeTranscriptRequest {
                task_id: format!("batch-summary-{}", uuid::Uuid::new_v4()),
                config: llm_config,
                template,
                segments: sona_core::llm::jobs::summary_inputs_from_transcript(&current),
                chunk_char_budget: None,
            };
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                sona_application::llm::LlmTaskService::new(adapter).summarize(request, &()),
            )
            .await
            .map_err(|_| "Summary generation timed out".to_string())?
            .map_err(|error| error.to_string())?;
            crate::platform::agent_control::facade::history::save_summary(
                services,
                crate::platform::agent_control::facade::types::SaveSummaryRequest {
                    history_id: history_id.to_string(),
                    content: result.content,
                    template_id: Some(result.template_id),
                    thought: result.thought,
                },
            )
            .await?;
        }
    }

    let mut export_path = None;
    let export_directory = pipeline
        .export_directory
        .as_deref()
        .filter(|directory| !directory.trim().is_empty());
    if pipeline.auto_export
        && let Some(directory) = export_directory
    {
        ensure_active(&cancelled)?;
        let format = ExportFormat::parse(pipeline.export_format.as_deref().unwrap_or("txt"))
            .map_err(|error| error.to_string())?;
        let mode = if pipeline.auto_translate {
            ExportMode::Bilingual
        } else {
            ExportMode::Original
        };
        let base = Path::new(filename)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(filename);
        let prefix = pipeline
            .export_file_name_prefix
            .as_deref()
            .unwrap_or("")
            .trim();
        let name = sanitize_filename(if prefix.is_empty() {
            base.to_string()
        } else {
            format!("{prefix} {base}")
        });
        let extension = match format {
            ExportFormat::Json => "json",
            ExportFormat::Txt => "txt",
            ExportFormat::Srt => "srt",
            ExportFormat::Vtt => "vtt",
            ExportFormat::Md => "md",
        };
        let output_path = Path::new(directory)
            .join(format!("{name}.{extension}"))
            .to_string_lossy()
            .into_owned();
        let returned_path = output_path.clone();
        let export_segments = current.clone();
        tokio::task::spawn_blocking(move || {
            sona_export::export_transcript_file(ExportTranscriptFileRequest {
                segments: export_segments,
                format,
                mode,
                output_path: output_path.clone(),
                bilingual_order: None,
            })
            .map(|_| output_path)
            .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())??;
        export_path = Some(returned_path);
    }

    Ok(BatchPipelineResult {
        segments: current,
        export_path,
    })
}

fn ensure_active(cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("Pipeline task cancelled.".to_string())
    } else {
        Ok(())
    }
}

async fn persist_segments(
    services: &DesktopServices,
    history_id: &str,
    segments: Vec<TranscriptSegment>,
) -> Result<(), String> {
    let history_id = history_id.to_string();
    services
        .history
        .mutation_db(move |repository| {
            repository.update_transcript(HistoryUpdateTranscriptRequest {
                history_id,
                segments,
            })
        })
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn replacement_sets(
    config: &Value,
    pipeline: &EffectivePipelineSnapshot,
    project_id: Option<&str>,
) -> Result<Vec<TranscriptTextReplacementRuleSet>, String> {
    if config
        .get("dictionaryContent")
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
    {
        let content = crate::views::settings::vocabulary::effective_dictionary_content(config);
        let entries = crate::views::settings::vocabulary::parse_dictionary_entries(&content);
        let rules = entries
            .into_iter()
            .filter(|entry| {
                entry.scope_id == "global" || project_id.is_some_and(|id| entry.scope_id == id)
            })
            .filter_map(|entry| {
                entry
                    .raw
                    .split_once("->")
                    .or_else(|| entry.raw.split_once("=>"))
                    .map(|(from, to)| (from.to_string(), to.to_string()))
            })
            .filter_map(|(from, to)| {
                let from = from.trim();
                let to = to.trim();
                (!from.is_empty() && !to.is_empty()).then(|| TranscriptTextReplacementRule {
                    from: from.to_string(),
                    to: to.to_string(),
                })
            })
            .collect::<Vec<_>>();
        return Ok((!rules.is_empty())
            .then_some(TranscriptTextReplacementRuleSet {
                enabled: true,
                ignore_case: false,
                rules,
            })
            .into_iter()
            .collect());
    }
    let app =
        serde_json::from_value::<AppConfig>(config.clone()).map_err(|error| error.to_string())?;
    let sets = app.text_replacement_sets.unwrap_or_default();
    let mut selected = pipeline
        .replacement_set_ids
        .iter()
        .filter_map(|id| sets.iter().find(|set| &set.id == id))
        .cloned()
        .collect::<Vec<_>>();
    if !pipeline.custom_terms.is_empty() {
        let rules = pipeline
            .custom_terms
            .iter()
            .filter_map(|term| term.split_once("=>").or_else(|| term.split_once("->")))
            .filter_map(|(from, to)| {
                let from = from.trim();
                let to = to.trim();
                (!from.is_empty() && !to.is_empty()).then(|| TextReplacementRuleRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                })
            })
            .collect::<Vec<_>>();
        if !rules.is_empty() {
            selected.push(TextReplacementSetRecord {
                id: "project-terms".into(),
                name: "Project Terms".into(),
                enabled: true,
                ignore_case: false,
                rules,
            });
        }
    }
    Ok(selected
        .into_iter()
        .map(|set| TranscriptTextReplacementRuleSet {
            enabled: set.enabled,
            ignore_case: set.ignore_case,
            rules: set
                .rules
                .into_iter()
                .map(|rule| TranscriptTextReplacementRule {
                    from: rule.from,
                    to: rule.to,
                })
                .collect(),
        })
        .collect())
}

fn polish_context(config: &Value, pipeline: &EffectivePipelineSnapshot) -> Option<String> {
    pipeline
        .polish_prompt_override
        .clone()
        .or_else(|| {
            let presets = config.get("polishCustomPresets")?.as_array()?;
            let id = pipeline
                .polish_preset_id
                .as_deref()
                .or_else(|| config.get("polishPresetId")?.as_str())?;
            presets
                .iter()
                .find(|preset| preset.get("id").and_then(Value::as_str) == Some(id))?
                .get("context")?
                .as_str()
                .map(str::to_owned)
        })
        .or_else(|| {
            config
                .get("polishContext")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn summary_template(config: &Value, id: &str) -> SummaryTemplateConfig {
    config.get("summaryCustomTemplates").and_then(Value::as_array).and_then(|templates| templates.iter().find(|template| template.get("id").and_then(Value::as_str) == Some(id))).map(|template| SummaryTemplateConfig {
        id: id.to_string(), name: template.get("name").and_then(Value::as_str).unwrap_or("Custom").to_string(), instructions: template.get("instructions").and_then(Value::as_str).unwrap_or_default().to_string(),
    }).unwrap_or_else(|| SummaryTemplateConfig { id: "general".into(), name: "General".into(), instructions: "1. A short overview paragraph.\n2. A concise list of key points.\n3. Follow-up items only when supported by the transcript.".into() })
}

fn sanitize_filename(name: String) -> String {
    name.chars()
        .map(|character| {
            if r#"\/:*?"<>|"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::replacement_sets;
    use serde_json::json;
    use sona_core::project::EffectivePipelineSnapshot;

    fn pipeline() -> EffectivePipelineSnapshot {
        EffectivePipelineSnapshot {
            is_project_pipeline: true,
            auto_polish: false,
            polish_preset_id: None,
            polish_prompt_override: None,
            auto_translate: false,
            target_language: None,
            auto_summary: false,
            summary_template_id: None,
            hotword_set_ids: Vec::new(),
            replacement_set_ids: Vec::new(),
            custom_terms: vec!["K8s -> Kubernetes".into()],
            auto_export: false,
            export_format: None,
            export_directory: None,
            export_file_name_prefix: None,
        }
    }

    #[test]
    fn unified_dictionary_replacements_are_limited_to_assigned_project() {
        let config = json!({
            "dictionaryContent": "- global -> Global\nprojects:\n  Research (id:p1):\n    - local -> Local\n  Other (id:p2):\n    - private -> Private"
        });
        let sets = replacement_sets(&config, &pipeline(), Some("p1")).unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].rules.len(), 2);
        assert_eq!(sets[0].rules[0].from, "global");
        assert_eq!(sets[0].rules[1].to, "Local");
    }

    #[test]
    fn legacy_pipeline_selects_configured_sets_and_custom_replacements() {
        let config = json!({
            "textReplacementSets": [{
                "id": "set-1",
                "name": "Names",
                "enabled": true,
                "ignoreCase": true,
                "rules": [{"id": "r1", "from": "sona ai", "to": "Sona"}]
            }]
        });
        let mut pipeline = pipeline();
        pipeline.replacement_set_ids = vec!["set-1".into()];
        let sets = replacement_sets(&config, &pipeline, None).unwrap();
        assert_eq!(sets.len(), 2);
        assert!(sets[0].ignore_case);
        assert_eq!(sets[1].rules[0].from, "K8s");
    }
}
