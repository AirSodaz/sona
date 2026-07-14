use std::collections::HashMap;

use crate::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};

pub fn merge_model_metadata(
    discovered: Vec<LlmModelSummary>,
    catalog: Vec<LlmModelSummary>,
) -> Vec<LlmModelSummary> {
    let catalog = catalog
        .into_iter()
        .map(|model| (model.model.clone(), model))
        .collect::<HashMap<_, _>>();

    discovered
        .into_iter()
        .map(|mut model| {
            if model.metadata_sources.is_empty() {
                model
                    .metadata_sources
                    .push(LlmModelMetadataSource::Provider);
            }
            if let Some(metadata) = catalog.get(&model.model) {
                merge_summary(&mut model, metadata);
            }
            derive_multimodal_support(&mut model);
            model
        })
        .collect()
}

fn merge_summary(target: &mut LlmModelSummary, source: &LlmModelSummary) {
    fill(&mut target.display_name, &source.display_name);
    fill(&mut target.input_price, &source.input_price);
    fill(&mut target.output_price, &source.output_price);
    fill(&mut target.cache_read_price, &source.cache_read_price);
    fill(&mut target.cache_write_price, &source.cache_write_price);
    fill(&mut target.context_window, &source.context_window);
    fill(&mut target.max_output_tokens, &source.max_output_tokens);
    fill(&mut target.knowledge_cutoff, &source.knowledge_cutoff);
    fill(&mut target.release_date, &source.release_date);
    fill(&mut target.last_updated, &source.last_updated);
    fill(&mut target.supports_multimodal, &source.supports_multimodal);
    fill(&mut target.supports_tools, &source.supports_tools);
    fill(&mut target.supports_reasoning, &source.supports_reasoning);
    fill(
        &mut target.supports_structured_output,
        &source.supports_structured_output,
    );
    fill(
        &mut target.supports_prompt_caching,
        &source.supports_prompt_caching,
    );
    if target.input_modalities.is_empty() {
        target.input_modalities.clone_from(&source.input_modalities);
    }
    if target.output_modalities.is_empty() {
        target
            .output_modalities
            .clone_from(&source.output_modalities);
    }
    for metadata_source in &source.metadata_sources {
        if !target.metadata_sources.contains(metadata_source) {
            target.metadata_sources.push(*metadata_source);
        }
    }
}

fn fill<T: Clone>(target: &mut Option<T>, source: &Option<T>) {
    if target.is_none() {
        target.clone_from(source);
    }
}

fn derive_multimodal_support(model: &mut LlmModelSummary) {
    if model.supports_multimodal.is_some()
        || (model.input_modalities.is_empty() && model.output_modalities.is_empty())
    {
        return;
    }
    let has_non_text = model
        .input_modalities
        .iter()
        .chain(&model.output_modalities)
        .any(|modality| *modality != LlmModality::Text);
    model.supports_multimodal = Some(has_non_text);
}
