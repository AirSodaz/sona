use std::time::Duration;

use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};
use sona_online_llm::{ModelsDevCatalog, parse_models_dev_models, should_enrich_model_metadata};

const CATALOG: &str = r#"{
  "openai": {
    "id": "openai",
    "models": {
      "gpt-test": {
        "id": "gpt-test",
        "name": "GPT Test",
        "structured_output": true,
        "reasoning": false,
        "tool_call": true,
        "knowledge": "2024-06",
        "release_date": "2025-01-02",
        "last_updated": "2025-02-03",
        "modalities": {"input": ["text", "image"], "output": ["text"]},
        "limit": {"context": 128000, "output": 16384},
        "cost": {"input": 2.0, "output": 8.0, "cache_read": 0.5}
      }
    }
  }
}"#;

#[test]
fn models_dev_parser_maps_exact_requested_models() {
    let models = parse_models_dev_models(CATALOG, "openai", &["gpt-test", "missing"]).unwrap();
    let model = &models[0];

    assert_eq!(models.len(), 1);
    assert_eq!(
        (
            model.display_name.as_deref(),
            model.context_window,
            model.max_output_tokens,
            model.knowledge_cutoff.as_deref(),
            model.supports_structured_output,
            model.supports_prompt_caching,
        ),
        (
            Some("GPT Test"),
            Some(128_000),
            Some(16_384),
            Some("2024-06"),
            Some(true),
            Some(true),
        )
    );
    assert_eq!(
        model.input_modalities,
        vec![LlmModality::Text, LlmModality::Image]
    );
    assert_eq!(
        model.metadata_sources,
        vec![LlmModelMetadataSource::ModelsDev]
    );
}

#[tokio::test]
async fn models_dev_failure_leaves_discovered_models_unchanged() {
    let catalog =
        ModelsDevCatalog::with_endpoint("http://127.0.0.1:1/api.json", Duration::from_millis(50));
    let discovered = vec![LlmModelSummary {
        model: "gpt-test".into(),
        context_window: Some(200_000),
        ..LlmModelSummary::default()
    }];

    let enriched = catalog.enrich("openai", discovered.clone()).await;

    assert_eq!(enriched, discovered);
}

#[test]
fn models_dev_skips_private_providers_and_loopback_endpoints() {
    assert!(!should_enrich_model_metadata(
        &LlmProvider::Custom("private".into()),
        "https://gateway.example.com"
    ));
    assert!(!should_enrich_model_metadata(
        &LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        "http://127.0.0.1:1234"
    ));
    assert!(should_enrich_model_metadata(
        &LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        "https://api.openai.com"
    ));
}
