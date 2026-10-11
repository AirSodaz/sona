use std::sync::{Arc, Mutex};

use sona_archive::FsBackupArchiveRepository;

#[derive(Clone, Default)]
pub struct HistoryRepositoryState {
    /// Serializes history operations that combine SQLite state with filesystem
    /// side effects, such as audio promotion/removal and backup import/export.
    pub(crate) file_lock: Arc<Mutex<()>>,
}

impl HistoryRepositoryState {
    pub(crate) fn is_file_task_active(&self) -> bool {
        matches!(
            self.file_lock.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        )
    }
}

#[derive(Clone, Default)]
pub struct PreparedBackupImportState {
    archive: Arc<FsBackupArchiveRepository>,
}

impl PreparedBackupImportState {
    pub(crate) fn archive(&self) -> Arc<FsBackupArchiveRepository> {
        Arc::clone(&self.archive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_file_task_active_returns_false_when_unlocked() {
        let state = HistoryRepositoryState::default();
        assert!(!state.is_file_task_active());
    }

    #[test]
    fn is_file_task_active_returns_true_when_locked() {
        let state = HistoryRepositoryState::default();
        let _guard = match state.file_lock.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        assert!(state.is_file_task_active());
    }

    #[test]
    fn is_file_task_active_returns_false_when_mutex_poisoned() {
        let state = HistoryRepositoryState::default();
        let lock_clone = Arc::clone(&state.file_lock);

        // Induce lock poisoning via panic on another thread while holding the lock
        let handle = std::thread::spawn(move || {
            let _guard = match lock_clone.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            panic!("simulated panic to poison mutex");
        });
        let _ = handle.join();

        assert!(state.file_lock.is_poisoned());
        assert!(!state.is_file_task_active());
    }
}
