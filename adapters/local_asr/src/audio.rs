use std::path::{Path, PathBuf};

pub fn resolve_ffmpeg_sidecar_path_from_exe(exe_path: &Path) -> Result<PathBuf, String> {
    let exe_dir = exe_path
        .parent()
        .ok_or("Failed to get parent directory of executable")?;

    #[cfg(windows)]
    let ffmpeg_filename = "ffmpeg.exe";
    #[cfg(not(windows))]
    let ffmpeg_filename = "ffmpeg";

    Ok(exe_dir.join(ffmpeg_filename))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::resolve_ffmpeg_sidecar_path_from_exe;

    #[test]
    fn resolves_ffmpeg_sidecar_path_next_to_cli_executable() {
        let exe = Path::new("/tmp/sona-cli");
        let ffmpeg = resolve_ffmpeg_sidecar_path_from_exe(exe).unwrap();

        #[cfg(windows)]
        assert!(ffmpeg.ends_with("ffmpeg.exe"));

        #[cfg(not(windows))]
        assert!(ffmpeg.ends_with("ffmpeg"));
    }
}
