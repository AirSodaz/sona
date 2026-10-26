use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource};
use sona_online_llm::provider_discovery::{
    parse_anthropic_models, parse_gemini_models, parse_ollama_models, parse_openrouter_models,
};

#[test]
fn test_gemini_models_parser() {
    let json = r#"{
      "models": [
        {
          "name": "models/gemini-1.5-pro",
          "displayName": "Gemini 1.5 Pro",
          "inputTokenLimit": 2097152,
          "outputTokenLimit": 8192,
          "supportedGenerationMethods": ["generateContent", "countTokens"]
        },
        {
          "name": "models/text-embedding-004",
          "displayName": "Text Embedding 004",
          "inputTokenLimit": 2048,
          "outputTokenLimit": 0,
          "supportedGenerationMethods": ["embedContent"]
        }
      ]
    }"#;

    let models = parse_gemini_models(json).expect("should parse gemini response");
    assert_eq!(models.len(), 2);

    let pro = &models[0];
    assert_eq!(pro.model, "gemini-1.5-pro");
    assert_eq!(pro.display_name.as_deref(), Some("Gemini 1.5 Pro"));
    assert_eq!(pro.context_window, Some(2097152));
    assert_eq!(pro.max_output_tokens, Some(8192));
    assert_eq!(pro.supports_tools, Some(true));
    assert_eq!(pro.supports_multimodal, Some(true));
    assert_eq!(pro.metadata_sources, vec![LlmModelMetadataSource::Provider]);

    let embed = &models[1];
    assert_eq!(embed.model, "text-embedding-004");
    assert_eq!(embed.supports_tools, None);
    assert_eq!(embed.supports_multimodal, None);
}

#[test]
fn test_ollama_models_parser() {
    let json = r#"{
      "models": [
        {
          "name": "llama3:latest",
          "details": {
            "parameter_size": "8.0B",
            "quantization_level": "Q4_0"
          }
        },
        {
          "name": "mistral:latest",
          "details": {
            "parameter_size": "7.2B"
          }
        },
        {
          "name": "nometa:latest"
        }
      ]
    }"#;

    let models = parse_ollama_models(json).expect("should parse ollama response");
    assert_eq!(models.len(), 3);

    assert_eq!(models[0].model, "llama3:latest");
    assert_eq!(
        models[0].display_name.as_deref(),
        Some("llama3:latest (8.0B, Q4_0)")
    );
    assert_eq!(
        models[0].metadata_sources,
        vec![LlmModelMetadataSource::Provider]
    );

    assert_eq!(models[1].model, "mistral:latest");
    assert_eq!(
        models[1].display_name.as_deref(),
        Some("mistral:latest (7.2B)")
    );

    assert_eq!(models[2].model, "nometa:latest");
    assert_eq!(models[2].display_name, None);
}

#[test]
fn test_openrouter_models_parser() {
    let json = r#"{
      "data": [
        {
          "id": "anthropic/claude-3.5-sonnet",
          "name": "Anthropic: Claude 3.5 Sonnet",
          "context_length": 200000,
          "pricing": {
            "prompt": "0.000003",
            "completion": "0.000015"
          },
          "top_provider": {
            "max_completion_tokens": 8192
          },
          "architecture": {
            "modality": "text+image->text"
          }
        },
        {
          "id": "meta-llama/llama-3-8b",
          "name": "Meta: Llama 3 8B",
          "context_length": 8192,
          "pricing": {
            "prompt": 0.0000001,
            "completion": 0.0000002
          },
          "architecture": {
            "modality": "text->text"
          }
        }
      ]
    }"#;

    let models = parse_openrouter_models(json).expect("should parse openrouter response");
    assert_eq!(models.len(), 2);

    let sonnet = &models[0];
    assert_eq!(sonnet.model, "anthropic/claude-3.5-sonnet");
    assert_eq!(
        sonnet.display_name.as_deref(),
        Some("Anthropic: Claude 3.5 Sonnet")
    );
    assert_eq!(sonnet.context_window, Some(200000));
    assert_eq!(sonnet.max_output_tokens, Some(8192));
    assert_eq!(sonnet.input_price, Some(3.0));
    assert_eq!(sonnet.output_price, Some(15.0));
    assert_eq!(sonnet.supports_multimodal, Some(true));
    assert!(sonnet.input_modalities.contains(&LlmModality::Image));
    assert_eq!(
        sonnet.metadata_sources,
        vec![LlmModelMetadataSource::Provider]
    );

    let llama = &models[1];
    assert_eq!(llama.model, "meta-llama/llama-3-8b");
    assert_eq!(llama.context_window, Some(8192));
    assert_eq!(llama.input_price, Some(0.1));
    assert_eq!(llama.output_price, Some(0.2));
    assert_eq!(llama.supports_multimodal, None);
}

#[test]
fn test_anthropic_models_parser() {
    let json = r#"{
      "data": [
        {
          "id": "claude-3-5-sonnet-20241022",
          "display_name": "Claude 3.5 Sonnet",
          "created_at": "2024-10-22T00:00:00Z"
        }
      ]
    }"#;

    let models = parse_anthropic_models(json).expect("should parse anthropic response");
    assert_eq!(models.len(), 1);

    let model = &models[0];
    assert_eq!(model.model, "claude-3-5-sonnet-20241022");
    assert_eq!(model.display_name.as_deref(), Some("Claude 3.5 Sonnet"));
    assert_eq!(model.release_date.as_deref(), Some("2024-10-22T00:00:00Z"));
    assert_eq!(
        model.metadata_sources,
        vec![LlmModelMetadataSource::Provider]
    );
}
