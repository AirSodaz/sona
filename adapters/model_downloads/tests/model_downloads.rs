use axum::{Router, routing::get};
use hex::encode;
use sha2::{Digest, Sha256};
use sona_core::models::downloads::ResolvedModelDownload;
use sona_core::models::preset_models::find_preset_model;
use sona_model_downloads::{
    DownloadError, DownloadFileOperation, download_model, installed_model_is_valid,
    remove_model_install_path, sha256_file,
};
use tokio::net::TcpListener;

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    encode(hasher.finalize())
}

#[test]
fn remove_model_install_path_removes_files_and_directories() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("silero_vad.onnx");
    let directory_path = dir.path().join("sherpa-onnx-whisper-turbo");

    std::fs::write(&file_path, "fake").unwrap();
    std::fs::create_dir_all(&directory_path).unwrap();
    std::fs::write(directory_path.join("model.onnx"), "fake").unwrap();

    remove_model_install_path(&file_path).unwrap();
    remove_model_install_path(&directory_path).unwrap();
    remove_model_install_path(&dir.path().join("missing-model")).unwrap();

    assert!(!file_path.exists());
    assert!(!directory_path.exists());
}

#[test]
fn remove_model_install_path_reports_inspection_context() {
    let invalid_path = std::path::Path::new("invalid\0model");

    let error = remove_model_install_path(invalid_path).unwrap_err();

    let DownloadError::FileSystem(context) = error else {
        panic!("expected filesystem error");
    };
    assert_eq!(context.operation, DownloadFileOperation::InspectInstall);
    assert_eq!(context.path, invalid_path);
    assert_eq!(context.target, None);
}

#[tokio::test]
async fn model_hash_errors_preserve_install_path() {
    let dir = tempfile::tempdir().unwrap();
    let install_path = dir.path().join("missing-model.onnx");

    let error = sha256_file(&install_path).await.unwrap_err();

    let DownloadError::FileSystem(context) = error else {
        panic!("expected filesystem error");
    };
    assert_eq!(context.operation, DownloadFileOperation::HashFile);
    assert_eq!(context.path, install_path);
    assert_eq!(context.target, None);
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
