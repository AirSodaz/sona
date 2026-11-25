pub use crate::ports::path::{PathKind, PathPort};

use std::path::{Path, PathBuf};

/// Searches `start_dir` and up to `max_ancestor_hops` parent directories for `filename`.
pub fn find_file_in_ancestors(
    start_dir: &Path,
    filename: &str,
    max_ancestor_hops: usize,
) -> Option<PathBuf> {
    let mut dir = Some(start_dir);
    for _ in 0..=max_ancestor_hops {
        let current = dir?;
        let candidate = current.join(filename);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

/// Searches the current executable's directory and up to `max_ancestor_hops` parent directories for `filename`.
/// Useful for Cargo v2 build-dir layouts where test binaries sit under `target/<profile>/build/<pkg>/<hash>/out/`.
pub fn find_executable_ancestor_file(filename: &str, max_ancestor_hops: usize) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let start_dir = exe.parent()?;
    find_file_in_ancestors(start_dir, filename, max_ancestor_hops)
}

#[cfg(test)]
mod tests {
    use super::find_file_in_ancestors;

    #[test]
    fn finds_file_in_ancestor_within_hop_limit() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir
            .path()
            .join("build")
            .join("pkg")
            .join("hash")
            .join("out");
        std::fs::create_dir_all(&nested).unwrap();
        let target = dir.path().join("onnxruntime.dll");
        std::fs::write(&target, b"dll").unwrap();

        assert_eq!(
            find_file_in_ancestors(&nested, "onnxruntime.dll", 4),
            Some(target)
        );
        assert_eq!(find_file_in_ancestors(&nested, "onnxruntime.dll", 3), None);
    }
}
