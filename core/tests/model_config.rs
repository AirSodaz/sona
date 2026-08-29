use sona_core::models::config::ModelFileConfig;

#[test]
fn model_file_config_transport_shape_lives_in_core() {
    let value = serde_json::to_value(ModelFileConfig {
        conv_frontend: Some("frontend.onnx".to_string()),
        encoder_adaptor: Some("adaptor.onnx".to_string()),
        mmproj: Some("mmproj.gguf".to_string()),
        preprocessor: Some("preprocess.onnx".to_string()),
        uncached_decoder: Some("uncached_decode.onnx".to_string()),
        cached_decoder: Some("cached_decode.onnx".to_string()),
        merged_decoder: Some("decoder_model_merged.ort".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(value["convFrontend"], "frontend.onnx");
    assert_eq!(value["encoderAdaptor"], "adaptor.onnx");
    assert_eq!(value["mmproj"], "mmproj.gguf");
    assert_eq!(value["preprocessor"], "preprocess.onnx");
    assert_eq!(value["uncachedDecoder"], "uncached_decode.onnx");
    assert_eq!(value["cachedDecoder"], "cached_decode.onnx");
    assert_eq!(value["mergedDecoder"], "decoder_model_merged.ort");
    assert!(value.get("conv_frontend").is_none());
    assert!(value.get("encoder_adaptor").is_none());
    assert!(value.get("merged_decoder").is_none());
}
