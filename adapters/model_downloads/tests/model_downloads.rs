use axum::{Router, routing::get};
use hex::encode;
use sha2::{Digest, Sha256};
use sona_core::models::downloads::{ResolvedModelArtifact, ResolvedModelDownload};
use sona_core::models::preset_models::find_preset_model;
use sona_model_downloads::{
    DownloadError, DownloadFileOperation, ModelDownloadStage, download_model,
    download_model_with_cancel, installed_model_is_valid, remove_model_install_path, sha256_file,
};
use std::sync::{Arc, Mutex};
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
        artifacts: Vec::new(),
    };

    assert!(!installed_model_is_valid(&resolved).await.unwrap());

    let downloaded = download_model(&resolved, |_, _| {}).await.unwrap();
    assert_eq!(downloaded, install_path);
    assert_eq!(tokio::fs::read(&downloaded).await.unwrap(), body);
    assert!(installed_model_is_valid(&resolved).await.unwrap());
}

#[tokio::test]
async fn cancellable_model_download_reports_download_verify_and_install_stages() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let body = b"progress-model";
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
        models_dir,
        download_path: install_path.clone(),
        install_path,
        artifacts: Vec::new(),
    };
    let stages = Arc::new(Mutex::new(Vec::new()));
    let recorded_stages = stages.clone();

    download_model_with_cancel(
        &resolved,
        Arc::new(tokio::sync::Notify::new()),
        move |event| recorded_stages.lock().unwrap().push(event.stage),
    )
    .await
    .unwrap();

    let stages = stages.lock().unwrap();
    assert!(stages.contains(&ModelDownloadStage::Downloading));
    assert!(stages.contains(&ModelDownloadStage::Verifying));
    assert!(stages.contains(&ModelDownloadStage::Installing));
}

#[tokio::test]
async fn downloads_and_atomically_publishes_multi_file_model() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_body = b"fake-qwen-gguf";
    let mmproj_body = b"fake-mmproj-gguf";

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/model.gguf", get(move || async move { model_body }))
        .route("/mmproj.gguf", get(move || async move { mmproj_body }));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let model = find_preset_model("qwen3-asr-0.6b-q8-gguf").unwrap().clone();
    let install_path = models_dir.join(&model.id);
    let resolved = ResolvedModelDownload {
        model,
        models_dir: models_dir.clone(),
        download_path: install_path.clone(),
        install_path: install_path.clone(),
        artifacts: vec![
            ResolvedModelArtifact {
                url: format!("http://{addr}/model.gguf"),
                filename: "model.gguf".to_string(),
                sha256: sha256_hex(model_body),
                size_bytes: model_body.len() as u64,
                install_path: install_path.join("model.gguf"),
            },
            ResolvedModelArtifact {
                url: format!("http://{addr}/mmproj.gguf"),
                filename: "mmproj.gguf".to_string(),
                sha256: sha256_hex(mmproj_body),
                size_bytes: mmproj_body.len() as u64,
                install_path: install_path.join("mmproj.gguf"),
            },
        ],
    };

    let downloaded = download_model(&resolved, |_, _| {}).await.unwrap();

    assert_eq!(downloaded, install_path);
    assert_eq!(
        std::fs::read(downloaded.join("model.gguf")).unwrap(),
        model_body
    );
    assert_eq!(
        std::fs::read(downloaded.join("mmproj.gguf")).unwrap(),
        mmproj_body
    );
    assert!(installed_model_is_valid(&resolved).await.unwrap());
    assert!(
        !models_dir
            .join(format!("{}.installing", resolved.model.id))
            .exists()
    );
}

#[tokio::test]
async fn multi_file_hash_failure_preserves_previous_install() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_body = b"corrupt-model";

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/model.gguf", get(move || async move { model_body }));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let model = find_preset_model("qwen3-asr-0.6b-q8-gguf").unwrap().clone();
    let install_path = models_dir.join(&model.id);
    std::fs::create_dir_all(&install_path).unwrap();
    std::fs::write(install_path.join("previous"), b"keep").unwrap();
    let resolved = ResolvedModelDownload {
        model,
        models_dir,
        download_path: install_path.clone(),
        install_path: install_path.clone(),
        artifacts: vec![ResolvedModelArtifact {
            url: format!("http://{addr}/model.gguf"),
            filename: "model.gguf".to_string(),
            sha256: sha256_hex(b"expected-model"),
            size_bytes: model_body.len() as u64,
            install_path: install_path.join("model.gguf"),
        }],
    };

    let error = download_model(&resolved, |_, _| {}).await.unwrap_err();

    assert!(matches!(error, DownloadError::HashMismatch { .. }));
    assert_eq!(
        std::fs::read(install_path.join("previous")).unwrap(),
        b"keep"
    );
    assert!(!install_path.join("model.gguf").exists());
}
