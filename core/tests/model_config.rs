use sona_core::models::config::ModelFileConfig;

#[test]
fn model_file_config_transport_shape_lives_in_core() {
    let value = serde_json::to_value(ModelFileConfig {
        conv_frontend: Some("frontend.onnx".to_string()),
        encoder_adaptor: Some("adaptor.onnx".to_string()),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(value["convFrontend"], "frontend.onnx");
    assert_eq!(value["encoderAdaptor"], "adaptor.onnx");
    assert!(value.get("conv_frontend").is_none());
    assert!(value.get("encoder_adaptor").is_none());
}
