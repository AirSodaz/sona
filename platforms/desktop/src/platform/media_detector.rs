pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    sona_media_detector::check_media_formats(paths).await
}
