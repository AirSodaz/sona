use sona_core::llm::model_catalog::merge_model_metadata;
use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};

#[test]
fn catalog_fills_missing_metadata_without_overriding_provider_values() {
    let discovered = LlmModelSummary {
        model: "model-a".into(),
        context_window: Some(200_000),
        metadata_sources: vec![LlmModelMetadataSource::Provider],
        ..LlmModelSummary::default()
    };
    let catalog = LlmModelSummary {
        model: "model-a".into(),
        display_name: Some("Model A".into()),
        context_window: Some(128_000),
        max_output_tokens: Some(16_384),
        knowledge_cutoff: Some("2024-06".into()),
        input_modalities: vec![LlmModality::Text, LlmModality::Image],
        output_modalities: vec![LlmModality::Text],
        supports_structured_output: Some(true),
        metadata_sources: vec![LlmModelMetadataSource::ModelsDev],
        ..LlmModelSummary::default()
    };

    let merged = merge_model_metadata(vec![discovered], vec![catalog]);
    let model = &merged[0];

    assert_eq!(
        (
            model.context_window,
            model.max_output_tokens,
            model.knowledge_cutoff.as_deref(),
            model.supports_multimodal,
            model.supports_structured_output,
        ),
        (
            Some(200_000),
            Some(16_384),
            Some("2024-06"),
            Some(true),
            Some(true)
        )
    );
    assert_eq!(
        model.metadata_sources,
        vec![
            LlmModelMetadataSource::Provider,
            LlmModelMetadataSource::ModelsDev
        ]
    );
}

#[test]
fn catalog_never_adds_models_missing_from_provider_discovery() {
    let catalog_only = LlmModelSummary {
        model: "catalog-only".into(),
        display_name: Some("Catalog Only".into()),
        ..LlmModelSummary::default()
    };

    assert!(merge_model_metadata(Vec::new(), vec![catalog_only]).is_empty());
}
