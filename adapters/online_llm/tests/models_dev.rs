use std::time::Duration;

use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};
use sona_core::ports::llm::LlmPortErrorKind;
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

#[test]
fn models_dev_parser_classifies_invalid_catalog_as_protocol_error() {
    let error = parse_models_dev_models("{broken", "openai", &["gpt-test"]).unwrap_err();

    assert_eq!(error.kind, LlmPortErrorKind::Protocol);
    assert!(error.message.contains("expected"));
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
fn models_dev_allows_online_gateways_and_skips_loopback_endpoints() {
    assert!(should_enrich_model_metadata(
        &LlmProvider::Custom("proxy".into()),
        "https://gateway.example.com"
    ));
    assert!(!should_enrich_model_metadata(
        &LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        "http://127.0.0.1:1234"
    ));
    assert!(!should_enrich_model_metadata(
        &LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        "http://192.168.1.50:1234"
    ));
    assert!(!should_enrich_model_metadata(
        &LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        "http://ollama.local:11434"
    ));
    assert!(should_enrich_model_metadata(
        &LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        "https://api.openai.com"
    ));
}

#[tokio::test]
async fn test_models_dev_disk_cache_persistence() {
    let temp_dir =
        std::env::temp_dir().join(format!("sona_models_dev_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::AsyncWriteExt;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                CATALOG.len(),
                CATALOG
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });

    let catalog = ModelsDevCatalog::with_endpoint_and_cache_dir(
        format!("http://{addr}/api.json"),
        Duration::from_secs(2),
        Some(temp_dir.clone()),
    );

    let discovered = vec![LlmModelSummary {
        model: "gpt-test".into(),
        ..LlmModelSummary::default()
    }];

    let enriched = catalog.enrich("openai", discovered).await;
    assert_eq!(enriched.len(), 1);
    assert_eq!(enriched[0].display_name.as_deref(), Some("GPT Test"));

    let cache_file = temp_dir.join("models_dev_cache.json");
    assert!(
        cache_file.is_file(),
        "models_dev_cache.json should be created on disk"
    );
    let content = std::fs::read_to_string(&cache_file).unwrap();
    assert!(content.contains("gpt-test"));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn test_models_dev_stale_on_error_recovery() {
    let temp_dir =
        std::env::temp_dir().join(format!("sona_models_dev_stale_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let cache_file = temp_dir.join("models_dev_cache.json");
    std::fs::write(&cache_file, CATALOG).unwrap();

    let catalog = ModelsDevCatalog::with_endpoint_and_cache_dir(
        "http://127.0.0.1:1/nonexistent.json",
        Duration::from_millis(50),
        Some(temp_dir.clone()),
    );

    let discovered = vec![LlmModelSummary {
        model: "gpt-test".into(),
        ..LlmModelSummary::default()
    }];

    let enriched = catalog.enrich("openai", discovered).await;
    assert_eq!(enriched.len(), 1);
    assert_eq!(enriched[0].display_name.as_deref(), Some("GPT Test"));
    assert_eq!(enriched[0].context_window, Some(128000));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_metadata_merge_priority_native_over_models_dev() {
    use sona_core::llm::model_catalog::merge_model_metadata;

    let catalog_models = parse_models_dev_models(CATALOG, "openai", &["gpt-test"]).unwrap();

    let discovered = vec![LlmModelSummary {
        model: "gpt-test".into(),
        display_name: Some("Native Discovered Name".into()),
        context_window: Some(256_000),
        max_output_tokens: None,
        metadata_sources: vec![LlmModelMetadataSource::Provider],
        ..LlmModelSummary::default()
    }];

    let merged = merge_model_metadata(discovered, catalog_models);
    assert_eq!(merged.len(), 1);
    let item = &merged[0];

    assert_eq!(item.display_name.as_deref(), Some("Native Discovered Name"));
    assert_eq!(item.context_window, Some(256_000));
    assert_eq!(item.max_output_tokens, Some(16384));
    assert_eq!(item.input_price, Some(2.0));
    assert_eq!(item.output_price, Some(8.0));

    assert!(
        item.metadata_sources
            .contains(&LlmModelMetadataSource::Provider)
    );
    assert!(
        item.metadata_sources
            .contains(&LlmModelMetadataSource::ModelsDev)
    );
}
