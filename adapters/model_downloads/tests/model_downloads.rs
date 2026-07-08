use axum::{Router, routing::get};
use hex::encode;
use sha2::{Digest, Sha256};
use sona_core::model_downloads::ResolvedModelDownload;
use sona_core::preset_models::find_preset_model;
use sona_model_downloads::{download_model, installed_model_is_valid};
use tokio::net::TcpListener;

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    encode(hasher.finalize())
}

#[tokio::test]
async fn downloads_single_file_model_and_validates_existing_hash() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let body = b"fake-silero-vad";
    let hash = sha256_hex(body);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/model.onnx", get(move || async move { body }));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut model = find_preset_model("silero-vad").unwrap().clone();
    model.url = format!("http://{addr}/model.onnx");
    model.sha256 = Some(hash);
    let install_path = models_dir.join("silero_vad.onnx");
    let resolved = ResolvedModelDownload {
        model,
        models_dir: models_dir.clone(),
        download_path: install_path.clone(),
        install_path: install_path.clone(),
    };

    assert!(!installed_model_is_valid(&resolved).await.unwrap());

    let downloaded = download_model(&resolved, |_, _| {}).await.unwrap();
    assert_eq!(downloaded, install_path);
    assert_eq!(tokio::fs::read(&downloaded).await.unwrap(), body);
    assert!(installed_model_is_valid(&resolved).await.unwrap());
}
