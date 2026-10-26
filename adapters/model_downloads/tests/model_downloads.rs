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

    let model = find_preset_model("silero-vad").unwrap().clone();
    let install_path = models_dir.join("silero_vad.onnx");
    let resolved = ResolvedModelDownload {
        model,
        models_dir: models_dir.clone(),
        download_path: install_path.clone(),
        install_path: install_path.clone(),
        artifacts: vec![ResolvedModelArtifact {
            url: format!("http://{addr}/model.onnx"),
            filename: "silero_vad.onnx".to_string(),
            sha256: Some(hash),
            size_bytes: Some(body.len() as u64),
            install_path: install_path.clone(),
        }],
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

    let model = find_preset_model("silero-vad").unwrap().clone();
    let install_path = models_dir.join("silero_vad.onnx");
    let resolved = ResolvedModelDownload {
        model,
        models_dir,
        download_path: install_path.clone(),
        install_path: install_path.clone(),
        artifacts: vec![ResolvedModelArtifact {
            url: format!("http://{addr}/model.onnx"),
            filename: "silero_vad.onnx".to_string(),
            sha256: Some(hash),
            size_bytes: Some(body.len() as u64),
            install_path: install_path.clone(),
        }],
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
                sha256: Some(sha256_hex(model_body)),
                size_bytes: Some(model_body.len() as u64),
                install_path: install_path.join("model.gguf"),
            },
            ResolvedModelArtifact {
                url: format!("http://{addr}/mmproj.gguf"),
                filename: "mmproj.gguf".to_string(),
                sha256: Some(sha256_hex(mmproj_body)),
                size_bytes: Some(mmproj_body.len() as u64),
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
            sha256: Some(sha256_hex(b"expected-model")),
            size_bytes: Some(model_body.len() as u64),
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

#[test]
fn inspect_and_clean_interrupted_download_single_file() {
    use sona_model_downloads::{clean_interrupted_download, inspect_interrupted_download};

    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();

    let model_id = "silero-vad";
    // Initially, no interrupted download
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());

    // Create a partial .download file
    let temp_file = models_dir.join("silero_vad.onnx.download");
    std::fs::write(&temp_file, b"partial-data").unwrap();

    let info = inspect_interrupted_download(&models_dir, model_id)
        .expect("should find interrupted download");
    assert_eq!(info.model_id, model_id);
    assert_eq!(info.downloaded_bytes, b"partial-data".len() as u64);
    assert!(info.total_bytes.is_some());

    // Clean interrupted download
    let cleaned = clean_interrupted_download(&models_dir, model_id).unwrap();
    assert!(cleaned);
    assert!(!temp_file.exists());
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());
}

#[test]
fn inspect_and_clean_interrupted_download_multi_file() {
    use sona_model_downloads::{clean_interrupted_download, inspect_interrupted_download};

    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();

    let model_id = "qwen3-asr-0.6b-q8-gguf";
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());

    // Create staging directory with partial download
    let staging_dir = models_dir.join(format!("{}.installing", model_id));
    std::fs::create_dir_all(&staging_dir).unwrap();
    let staged_temp = staging_dir.join("Qwen3-ASR-0.6B-Q8_0.gguf.download");
    std::fs::write(&staged_temp, b"multi-partial-bytes").unwrap();

    let info = inspect_interrupted_download(&models_dir, model_id)
        .expect("should find interrupted download");
    assert_eq!(info.model_id, model_id);
    assert_eq!(info.downloaded_bytes, b"multi-partial-bytes".len() as u64);

    // Clean interrupted download
    let cleaned = clean_interrupted_download(&models_dir, model_id).unwrap();
    assert!(cleaned);
    assert!(!staging_dir.exists());
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());
}

#[test]
fn inspect_and_clean_interrupted_download_local_llm() {
    use sona_model_downloads::{clean_interrupted_download, inspect_interrupted_download};

    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();

    let model_id = "qwen3.5-4b";
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());

    // Create partial .download file for the local LLM model
    let temp_file = models_dir.join("Qwen3.5-4B-Q4_K_M.gguf.download");
    std::fs::write(&temp_file, b"llm-partial-bytes").unwrap();

    let info = inspect_interrupted_download(&models_dir, model_id)
        .expect("should find interrupted LLM download");
    assert_eq!(info.model_id, model_id);
    assert_eq!(info.downloaded_bytes, b"llm-partial-bytes".len() as u64);
    assert!(info.total_bytes.is_some());

    let cleaned = clean_interrupted_download(&models_dir, model_id).unwrap();
    assert!(cleaned);
    assert!(!temp_file.exists());
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());
}

#[test]
fn inspect_and_clean_interrupted_download_handles_locked_files() {
    use fs3::FileExt;
    use sona_model_downloads::{clean_interrupted_download, inspect_interrupted_download};

    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();

    let model_id = "silero-vad";
    let temp_file = models_dir.join("silero_vad.onnx.download");
    std::fs::write(&temp_file, b"actively-downloading-data").unwrap();

    // Lock the file exclusively as an active download would
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&temp_file)
        .unwrap();
    file.try_lock_exclusive().unwrap();

    // While locked, inspection must return None (not interrupted)
    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());

    // Cleaning while locked must return Err(ResourceBusy)
    let clean_res = clean_interrupted_download(&models_dir, model_id);
    assert!(clean_res.is_err());
    assert_eq!(
        clean_res.unwrap_err().kind(),
        std::io::ErrorKind::ResourceBusy
    );
    assert!(temp_file.exists());

    // Unlock file
    drop(file);

    // Now inspection finds it
    assert!(inspect_interrupted_download(&models_dir, model_id).is_some());
    // And cleaning succeeds
    assert!(clean_interrupted_download(&models_dir, model_id).unwrap());
    assert!(!temp_file.exists());
}

#[test]
fn clean_interrupted_download_handles_installing_file_and_archive_staging() {
    use sona_model_downloads::{clean_interrupted_download, inspect_interrupted_download};

    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();

    // sensevoice is an archive model (artifacts.len() == 1, is_archive = true)
    let model_id = "sensevoice";
    let resolved =
        sona_core::models::downloads::resolve_model_download(model_id, &models_dir).unwrap();
    let staging_dir = sona_core::models::downloads::staging_install_path(&resolved.install_path);
    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::write(staging_dir.join("extracted_part.bin"), b"staged-bytes").unwrap();

    let info = inspect_interrupted_download(&models_dir, model_id);
    assert!(info.is_some());

    let cleaned = clean_interrupted_download(&models_dir, model_id).unwrap();
    assert!(cleaned);
    assert!(!staging_dir.exists());

    // Also test when .installing is a regular file (e.g. symlink or touched file)
    let staging_file = staging_dir;
    std::fs::write(&staging_file, b"stray-file").unwrap();
    assert!(staging_file.exists());

    let cleaned2 = clean_interrupted_download(&models_dir, model_id).unwrap();
    assert!(cleaned2);
    assert!(!staging_file.exists());
}

#[test]
fn inspect_and_clean_interrupted_download_handles_nested_locked_files() {
    use fs3::FileExt;
    use sona_model_downloads::{clean_interrupted_download, inspect_interrupted_download};

    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();

    let model_id = "qwen3-asr-0.6b-q8-gguf";
    let resolved =
        sona_core::models::downloads::resolve_model_download(model_id, &models_dir).unwrap();
    let staging_dir = sona_core::models::downloads::staging_install_path(&resolved.install_path);
    let nested_dir = staging_dir.join("tokenizer");
    std::fs::create_dir_all(&nested_dir).unwrap();
    let nested_file = nested_dir.join("vocab.json.download");
    std::fs::write(&nested_file, b"actively-downloading-nested-data").unwrap();

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&nested_file)
        .unwrap();
    file.try_lock_exclusive().unwrap();

    assert!(inspect_interrupted_download(&models_dir, model_id).is_none());

    let clean_res = clean_interrupted_download(&models_dir, model_id);
    assert!(clean_res.is_err());
    assert_eq!(
        clean_res.unwrap_err().kind(),
        std::io::ErrorKind::ResourceBusy
    );
    assert!(nested_file.exists());

    drop(file);

    assert!(clean_interrupted_download(&models_dir, model_id).unwrap());
    assert!(!staging_dir.exists());
}

#[tokio::test]
async fn cancel_during_connection_handshake_aborts_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model = find_preset_model("silero-vad").unwrap().clone();
    let install_path = models_dir.join(&model.id);

    // Set up a listener that accepts connections but never sends an HTTP response (simulating hanging connection)
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            drop(socket);
        }
    });

    let resolved = ResolvedModelDownload {
        model,
        models_dir: models_dir.clone(),
        download_path: install_path.clone(),
        install_path: install_path.clone(),
        artifacts: vec![ResolvedModelArtifact {
            url: format!("http://{addr}/silero_vad.onnx"),
            filename: "silero_vad.onnx".to_string(),
            sha256: None,
            size_bytes: Some(1000),
            install_path: install_path.clone(),
        }],
    };

    let cancel = Arc::new(tokio::sync::Notify::new());
    let cancel_for_task = cancel.clone();

    let download_task = tokio::spawn(async move {
        download_model_with_cancel(&resolved, cancel_for_task, |_| {}).await
    });

    // Allow time for request.send() to begin connecting
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let start = std::time::Instant::now();
    cancel.notify_one();

    let result = download_task.await.unwrap();
    let elapsed = start.elapsed();

    assert!(matches!(result, Err(DownloadError::Cancelled)));
    assert!(
        elapsed.as_millis() < 1000,
        "Cancellation took too long during connect phase: {:?}",
        elapsed
    );
}
