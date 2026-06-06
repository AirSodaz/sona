const EXTRACT_PROGRESS_EVENT: &str = "extract-progress";

#[tauri::command]
pub async fn extract_tar_bz2<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    use std::path::Path;
    use std::time::Instant;
    use tauri::Emitter;

    tauri::async_runtime::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|e| e.to_string())?;
        let buffered = std::io::BufReader::new(file);
        let tar = bzip2::read::BzDecoder::new(buffered);
        let mut archive = tar::Archive::new(tar);
        let target_path = Path::new(&target_dir);

        let mut last_emit = Instant::now();

        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;

            if last_emit.elapsed().as_millis() > 100 {
                let path = entry.path().map_err(|e| e.to_string())?;
                let path_str = path.to_string_lossy().to_string();
                let _ = app.emit(EXTRACT_PROGRESS_EVENT, &path_str);
                last_emit = Instant::now();
            }

            entry.unpack_in(target_path).map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    use std::fs::{self, File};
    use std::io::BufWriter;
    use std::path::{Path, PathBuf};

    fn append_directory_contents(
        builder: &mut tar::Builder<bzip2::write::BzEncoder<BufWriter<File>>>,
        root: &Path,
        current: &Path,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;

            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                builder
                    .append_dir(relative, &path)
                    .map_err(|e| e.to_string())?;
                append_directory_contents(builder, root, &path)?;
                continue;
            }

            builder
                .append_path_with_name(&path, relative)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    tauri::async_runtime::spawn_blocking(move || {
        let source_path = PathBuf::from(&source_dir);
        if !source_path.is_dir() {
            return Err(format!("Source directory does not exist: {}", source_dir));
        }

        let archive_path = PathBuf::from(&archive_path);
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let file = File::create(&archive_path).map_err(|e| e.to_string())?;
        let writer = BufWriter::new(file);
        let encoder = bzip2::write::BzEncoder::new(writer, bzip2::Compression::best());
        let mut builder = tar::Builder::new(encoder);

        append_directory_contents(&mut builder, &source_path, &source_path)?;

        let encoder = builder.into_inner().map_err(|e| e.to_string())?;
        encoder.finish().map_err(|e| e.to_string())?;

        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}
